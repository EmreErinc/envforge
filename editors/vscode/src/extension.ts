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

    // Now check binary
    const binaryPath = getEnvforgePath();
    const binaryExists = await checkBinary(binaryPath);

    if (!binaryExists) {
        const msg = `EnvForge binary not found. Install: cargo install env-forge-tui`;
        outputChannel.appendLine(`ERROR: ${msg}`);
        vscode.window.showWarningMessage(
            msg,
            'Set Path',
            'Open Settings',
        ).then(action => {
            if (action === 'Set Path') {
                vscode.window.showInputBox({
                    prompt: 'Full path to envforge binary',
                    placeHolder: '/Users/you/.cargo/bin/envforge',
                }).then(p => {
                    if (p) {
                        vscode.workspace.getConfiguration('envforge').update('path', p, true);
                        vscode.window.showInformationMessage('Path set. Reload window to apply.');
                    }
                });
            } else if (action === 'Open Settings') {
                vscode.commands.executeCommand('workbench.action.openSettings', 'envforge.path');
            }
        });
        outputChannel.appendLine('Extension activated (binary not found — limited mode)');
        return;
    }

    outputChannel.appendLine(`Binary found: ${binaryPath}`);

    // LSP
    const config = vscode.workspace.getConfiguration('envforge');
    if (config.get<boolean>('lsp.enable', true)) {
        try {
            client = await startLanguageServer(context, binaryPath);
            outputChannel.appendLine('Language Server started');
        } catch (e: any) {
            outputChannel.appendLine(`LSP start failed: ${e.message}`);
            vscode.window.showErrorMessage(`EnvForge LSP failed: ${e.message}`);
        }
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
            // Source-language selectors enable LSP requests (notably
            // goto-definition) from code that reads env vars. The LSP
            // server silently ignores did_open/did_change for these
            // URIs; they only participate in definition lookups.
            { scheme: 'file', language: 'typescript' },
            { scheme: 'file', language: 'typescriptreact' },
            { scheme: 'file', language: 'javascript' },
            { scheme: 'file', language: 'javascriptreact' },
            { scheme: 'file', language: 'python' },
            { scheme: 'file', language: 'rust' },
            { scheme: 'file', language: 'go' },
            { scheme: 'file', language: 'java' },
            { scheme: 'file', language: 'kotlin' },
            { scheme: 'file', language: 'ruby' },
            { scheme: 'file', language: 'php' },
            { scheme: 'file', language: 'csharp' },
            { scheme: 'file', language: 'shellscript' },
            // MCP config files — linted inline for hardcoded credentials.
            { scheme: 'file', pattern: '**/mcp.json' },
            { scheme: 'file', pattern: '**/.mcp.json' },
            { scheme: 'file', pattern: '**/.cursor/mcp.json' },
            { scheme: 'file', pattern: '**/.claude/settings.json' },
            { scheme: 'file', pattern: '**/claude_desktop_config.json' },
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
    const config = vscode.workspace.getConfiguration('envforge');
    const customPath = config.get<string>('path', '');
    if (customPath && fs.existsSync(customPath)) {
        return customPath;
    }

    // Check common install locations (absolute paths only — no PATH fallback)
    const home = os.homedir();
    const candidates = [
        path.join(home, '.cargo', 'bin', 'envforge'),
        '/usr/local/bin/envforge',
        '/opt/homebrew/bin/envforge',
    ];

    for (const p of candidates) {
        if (fs.existsSync(p)) {
            return p;
        }
    }

    // No PATH fallback — binary must be found at an absolute path
    // to prevent supply-chain attacks via PATH manipulation.
    return '';
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
