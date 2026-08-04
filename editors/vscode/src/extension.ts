import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import * as cp from 'child_process';
import { LanguageClient, LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node';
import { registerCommands } from './commands';
import { StatusBar } from './statusbar';
import { EnvTreeProvider, ProfileTreeProvider } from './treeview';
import { SecurityTreeProvider, registerSecurityCommands } from './security';
import { ExposureRenderer } from './exposure';
import { EnvFileDecorationProvider } from './decorations';
import { WelcomeWebviewPanel } from './welcome';

import { findBinaryPath, ensureBinaryWithProgress } from './binaryManager';

let client: LanguageClient | undefined;
let statusBar: StatusBar;
let outputChannel: vscode.OutputChannel;
let treeProvider: EnvTreeProvider;
let profileProvider: ProfileTreeProvider;
let securityProvider: SecurityTreeProvider;

export async function activate(context: vscode.ExtensionContext) {
    outputChannel = vscode.window.createOutputChannel('EnvForge');
    context.subscriptions.push(outputChannel);
    outputChannel.appendLine('EnvForge extension activating...');

    // Always create UI components and register commands first
    statusBar = new StatusBar();
    context.subscriptions.push(statusBar);

    treeProvider = new EnvTreeProvider();
    context.subscriptions.push(
        vscode.window.createTreeView('envforgeVariables', { treeDataProvider: treeProvider })
    );

  profileProvider = new ProfileTreeProvider();
  context.subscriptions.push(
    vscode.window.createTreeView('envforgeProfiles', { treeDataProvider: profileProvider })
  );

  securityProvider = new SecurityTreeProvider();
  context.subscriptions.push(
    vscode.window.createTreeView('envforgeSecurity', { treeDataProvider: securityProvider })
  );

  registerCommands(context, statusBar, treeProvider, profileProvider);
  registerSecurityCommands(context, securityProvider, treeProvider, statusBar);

  // Auto-refresh security tree on .env file save
  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument(doc => {
      if (doc.uri.scheme === 'file' && doc.fileName.match(/\.env(\.|$)/)) {
        securityProvider.refresh();
      }
    })
  );

    // Show welcome panel only on first extension installation/launch
    const WELCOME_SHOWN_KEY = 'envforge.welcomeShown';
    const welcomeShown = context.globalState.get<boolean>(WELCOME_SHOWN_KEY, false);
    if (!welcomeShown) {
        await context.globalState.update(WELCOME_SHOWN_KEY, true);
        WelcomeWebviewPanel.show(context);
    }

    // Now check binary (or auto-download if missing)
    let binaryPath = getEnvforgePath();
    let binaryExists = await checkBinary(binaryPath);

    if (!binaryExists) {
        outputChannel.appendLine('Binary not found in standard paths. Attempting auto-download...');
        const downloaded = await ensureBinaryWithProgress();
        if (downloaded) {
            binaryPath = getEnvforgePath();
            binaryExists = await checkBinary(binaryPath);
        }
    }

    await vscode.commands.executeCommand('setContext', 'envforge.cliAvailable', binaryExists);

    if (!binaryExists) {
        const installCmd = 'cargo install env-forge-tui';
        const msg = `EnvForge CLI is not installed. Click 'Auto-Download' or run '${installCmd}'.`;
        outputChannel.appendLine(`ERROR: ${msg}`);

        vscode.window.showWarningMessage(
            msg,
            'Auto-Download CLI',
            'Install CLI (Webview)',
            'Copy Command'
        ).then(async action => {
            if (action === 'Auto-Download CLI') {
                const ok = await ensureBinaryWithProgress();
                if (ok) {
                    vscode.commands.executeCommand('workbench.action.reloadWindow');
                }
            } else if (action === 'Install CLI (Webview)') {
                WelcomeWebviewPanel.show(context);
            } else if (action === 'Copy Command') {
                vscode.env.clipboard.writeText(installCmd);
                vscode.window.showInformationMessage(`Copied '${installCmd}' to clipboard!`);
            }
        });
        outputChannel.appendLine('Extension activated (binary not found — limited mode)');
        return;
    }

    outputChannel.appendLine(`Binary found: ${binaryPath}`);

    // LSP — gated on workspace trust (H5). In an untrusted workspace we must
    // not spawn the envforge binary or let it write fence files. Start (or
    // re-start) the language server only once the workspace is trusted.
    const config = vscode.workspace.getConfiguration('envforge');
    const startLsp = async () => {
        if (!config.get<boolean>('lsp.enable', true)) {
            return;
        }
        try {
            client = await startLanguageServer(context, binaryPath);
            outputChannel.appendLine('Language Server started');
        } catch (e: any) {
            outputChannel.appendLine(`LSP start failed: ${e.message}`);
            vscode.window.showErrorMessage(`EnvForge LSP failed: ${e.message}`);
        }
    };
    if (vscode.workspace.isTrusted) {
        await startLsp();
    } else {
        outputChannel.appendLine(
            'Workspace not trusted — EnvForge language server and binary are disabled (limited mode). Trust the workspace to enable them.'
        );
        context.subscriptions.push(
            vscode.workspace.onDidGrantWorkspaceTrust(() => {
                outputChannel.appendLine('Workspace trust granted — starting EnvForge language server.');
                void startLsp();
            })
        );
    }

    // AI-exposure heatmap renderer — drives `.env*` gutter glyphs off
    // the LSP `envforge/exposureMap` custom request.
    const exposureRenderer = new ExposureRenderer();
    context.subscriptions.push(exposureRenderer);
    context.subscriptions.push(
        vscode.window.onDidChangeActiveTextEditor(editor =>
            exposureRenderer.scheduleRefresh(editor)
        )
    );
    context.subscriptions.push(
        vscode.workspace.onDidChangeTextDocument(e => {
            const editor = vscode.window.activeTextEditor;
            if (editor && editor.document === e.document) {
                exposureRenderer.scheduleRefresh(editor);
            }
        })
    );
    context.subscriptions.push(
        vscode.workspace.onDidSaveTextDocument(doc => {
            const editor = vscode.window.activeTextEditor;
            if (editor && editor.document === doc) {
                exposureRenderer.scheduleRefresh(editor);
            }
        })
    );
    // Kick off initial render for whichever editor is active at startup.
    exposureRenderer.scheduleRefresh(vscode.window.activeTextEditor);

    // File explorer / open-tabs badges for .env* files. Reuses the
    // `envforge exposure` CLI so the badge stays consistent with the
    // in-editor gutter heatmap.
    const decorationProvider = new EnvFileDecorationProvider();
    context.subscriptions.push(decorationProvider);
    context.subscriptions.push(
        vscode.window.registerFileDecorationProvider(decorationProvider),
    );
    context.subscriptions.push(
        vscode.workspace.onDidSaveTextDocument(doc => {
            if (doc.uri.scheme === 'file' && /\.env(\.|$)/.test(doc.fileName)) {
                decorationProvider.refresh(doc.uri);
            }
        }),
    );
    // Fence toggle changes the workspace-wide badge story. Refresh
    // every cached entry after either fence command runs.
    context.subscriptions.push(
        vscode.commands.registerCommand('envforge.decorations.refreshAll', () =>
            decorationProvider.refresh(),
        ),
    );

    // Refresh data
    statusBar.update();
    treeProvider.refresh();
    profileProvider.refresh();

    outputChannel.appendLine('EnvForge extension activated');
}

async function startLanguageServer(
    context: vscode.ExtensionContext,
    binaryPath: string,
): Promise<LanguageClient> {
    const serverOptions: ServerOptions = {
        command: binaryPath,
        args: ['lsp'],
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [
            { scheme: 'file', language: 'dotenv' },
            { scheme: 'file', language: 'env-schema' },
            { scheme: 'file', pattern: '**/.env' },
            { scheme: 'file', pattern: '**/.env.*' },
            { scheme: 'file', pattern: '**/*.env' },
            // NOTE: EnvForge does NOT attach to whole source languages
            // (java/rust/python/…). Sending every source file to the LSP to
            // provide one niche feature (goto from an env-var read back to
            // .env.schema) degrades the editor and breaks native code
            // intelligence in LSP4IJ-style clients. The LSP attaches only to
            // EnvForge's own files (env/schema + mcp).
            // The goto-from-source-code feature is intentionally dropped.
            // MCP config files — linted inline for hardcoded credentials.
            { scheme: 'file', pattern: '**/mcp.json' },
            { scheme: 'file', pattern: '**/.mcp.json' },
            { scheme: 'file', pattern: '**/.cursor/mcp.json' },
            { scheme: 'file', pattern: '**/.claude/settings.json' },
            { scheme: 'file', pattern: '**/claude_desktop_config.json' },
            // Story 3.1 (FR18): widened AI-tool/agent MCP config coverage.
            { scheme: 'file', pattern: '**/.claude.json' },          // Claude Code user config
            { scheme: 'file', pattern: '**/mcp_config.json' },       // Windsurf (Cascade)
            { scheme: 'file', pattern: '**/cline_mcp_settings.json' }, // Cline
        ],
        synchronize: {
            fileEvents: [
                vscode.workspace.createFileSystemWatcher('**/.env.schema'),
                vscode.workspace.createFileSystemWatcher('**/.env.schema.toml'),
                vscode.workspace.createFileSystemWatcher('**/.env'),
                vscode.workspace.createFileSystemWatcher('**/.env.*'),
            ],
        },
        outputChannel,
    };

    const lspClient = new LanguageClient(
        'envforge',
        'EnvForge Language Server',
        serverOptions,
        clientOptions,
    );

    await lspClient.start();
    context.subscriptions.push({ dispose: () => lspClient.stop() });

    return lspClient;
}

export function getEnvforgePath(): string {
    return findBinaryPath() || '';
}

async function checkBinary(binaryPath: string): Promise<boolean> {
    if (!binaryPath) {
        return false;
    }
    return new Promise((resolve) => {
        cp.execFile(binaryPath, ['--version'], { timeout: 5000 }, (err, stdout) => {
            if (err) {
                resolve(false);
            } else {
                resolve(stdout.includes('envforge'));
            }
        });
    });
}

export function getOutputChannel(): vscode.OutputChannel {
    return outputChannel;
}

export function getClient(): LanguageClient | undefined {
    return client;
}

export async function deactivate(): Promise<void> {
    if (client) {
        await client.stop();
    }
}
