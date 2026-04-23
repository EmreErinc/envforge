import * as vscode from 'vscode';
import * as cp from 'child_process';
import { getEnvforgePath, getOutputChannel } from './extension';
import { StatusBar } from './statusbar';
import { EnvTreeProvider, ProfileTreeProvider } from './treeview';

export function registerCommands(
    context: vscode.ExtensionContext,
    statusBar: StatusBar,
    treeProvider: EnvTreeProvider,
    profileProvider: ProfileTreeProvider,
) {
    // Commands that accept an optional URI argument (from explorer/editor context menus)
    const fileAwareCommands: [string, (uri?: vscode.Uri) => Promise<void>][] = [
        ['envforge.validate', cmdValidate],
        ['envforge.scan', cmdScan],
        ['envforge.schemaGenerate', cmdSchemaGenerate],
        ['envforge.export', cmdExport],
        ['envforge.check', cmdCheck],
    ];

    for (const [id, handler] of fileAwareCommands) {
        context.subscriptions.push(
            vscode.commands.registerCommand(id, handler)
        );
    }

    // Commands that do not need a URI argument
    const commands: [string, () => Promise<void>][] = [
        ['envforge.list', cmdList],
        ['envforge.profileSwitch', cmdProfileSwitch],
        ['envforge.profileDiff', cmdProfileDiff],
        ['envforge.syncStatus', cmdSyncStatus],
        ['envforge.syncPush', cmdSyncPush],
        ['envforge.syncPull', cmdSyncPull],
        ['envforge.doctor', cmdDoctor],
        ['envforge.restartLsp', cmdRestartLsp],
    ];

    for (const [id, handler] of commands) {
        context.subscriptions.push(
            vscode.commands.registerCommand(id, handler)
        );
    }

    // Tree view commands
    context.subscriptions.push(
        vscode.commands.registerCommand('envforge.refreshTree', () => {
            treeProvider.refresh();
            profileProvider.refresh();
        }),
        vscode.commands.registerCommand('envforge.copyValue', (arg: any) => {
            // Context menu passes VarNode, click passes string
            const value = arg?.envVar?.value ?? arg;
            if (typeof value === 'string') {
                vscode.env.clipboard.writeText(value);
                vscode.window.showInformationMessage('Value copied to clipboard');
            }
        }),
        vscode.commands.registerCommand('envforge.copyKey', (arg: any) => {
            const key = arg?.envVar?.key ?? arg;
            if (typeof key === 'string') {
                vscode.env.clipboard.writeText(key);
                vscode.window.showInformationMessage(`Key copied: ${key}`);
            }
        }),
        vscode.commands.registerCommand('envforge.copyKeyValue', (arg: any) => {
            const key = arg?.envVar?.key;
            const value = arg?.envVar?.value;
            if (typeof key === 'string' && typeof value === 'string') {
                vscode.env.clipboard.writeText(`${key}=${value}`);
                vscode.window.showInformationMessage(`Copied: ${key}=...`);
            }
        }),
        vscode.commands.registerCommand('envforge.profileContextSwitch', async (item: vscode.TreeItem) => {
            const name = typeof item?.label === 'string' ? item.label : (item?.label as any)?.label;
            if (!name) return;
            try {
                await run(['profile', 'switch', name]);
                vscode.window.showInformationMessage(`Switched to profile: ${name}`);
                profileProvider.refresh();
                treeProvider.refresh();
                statusBar.update();
            } catch (e: any) {
                vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
            }
        }),
        vscode.commands.registerCommand('envforge.profileContextDiff', async (item: vscode.TreeItem) => {
            const name = typeof item?.label === 'string' ? item.label : (item?.label as any)?.label;
            if (!name) return;
            try {
                // Find the active profile to diff against
                const { stdout } = await run(['profile', 'list', '--json']);
                const profiles: string[] = JSON.parse(stdout);
                // If this is the active profile, let user pick another
                if (item.contextValue === 'envProfileActive') {
                    const other = await vscode.window.showQuickPick(
                        profiles.filter(p => p !== name),
                        { placeHolder: 'Diff with profile' }
                    );
                    if (!other) return;
                    await runAndShow('Profile Diff', ['profile', 'diff', name, other]);
                } else {
                    // Diff inactive profile against the active one
                    // Find active profile name from tree description
                    await runAndShow('Profile Diff', ['profile', 'diff', name]);
                }
            } catch (e: any) {
                vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
            }
        }),
        vscode.commands.registerCommand('envforge.profileOpenFile', async (item: vscode.TreeItem) => {
            // description holds the file name for inactive profiles, 'active' for active
            const name = typeof item?.label === 'string' ? item.label : (item?.label as any)?.label;
            if (!name) return;
            const wsFolder = vscode.workspace.workspaceFolders?.[0];
            if (!wsFolder) return;
            // Profile file is typically .env.<name>
            const fileUri = vscode.Uri.joinPath(wsFolder.uri, `.env.${name}`);
            try {
                const doc = await vscode.workspace.openTextDocument(fileUri);
                await vscode.window.showTextDocument(doc);
            } catch {
                vscode.window.showErrorMessage(`Could not open file for profile: ${name}`);
            }
        }),
        vscode.commands.registerCommand('envforge.toggleGrouping', () => {
            treeProvider.toggleGrouping();
        }),
        vscode.commands.registerCommand('envforge.switchToProfile', async (name: string) => {
            try {
                await run(['profile', 'switch', name]);
                vscode.window.showInformationMessage(`Switched to profile: ${name}`);
                profileProvider.refresh();
                treeProvider.refresh();
                statusBar.update();
            } catch (e: any) {
                vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
            }
        }),
    );
}

// ── Helpers ──────────────────────────────────────────────────

function cwd(): string {
    return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || process.cwd();
}

/** Resolve working directory from an optional URI (file explorer context) */
function cwdFromUri(uri?: vscode.Uri): string {
    if (uri) {
        // If the URI points to a file, use its parent directory
        const stat = uri.fsPath;
        const path = require('path');
        return path.dirname(stat);
    }
    // Fallback to active editor's directory
    const activeUri = vscode.window.activeTextEditor?.document.uri;
    if (activeUri && activeUri.scheme === 'file') {
        const path = require('path');
        return path.dirname(activeUri.fsPath);
    }
    return cwd();
}

function run(args: string[], workingDir?: string): Promise<{ stdout: string; stderr: string }> {
    const binary = getEnvforgePath();
    const out = getOutputChannel();
    out.appendLine(`> ${binary} ${args.join(' ')}`);

    return new Promise((resolve, reject) => {
        cp.execFile(binary, args, { cwd: workingDir || cwd(), timeout: 30000 }, (err, stdout, stderr) => {
            if (err && !stdout) {
                const msg = stderr?.trim() || err.message;
                out.appendLine(`ERROR: ${msg}`);
                reject(new Error(msg));
            } else {
                resolve({ stdout: stdout || '', stderr: stderr || '' });
            }
        });
    });
}

function showOutput(title: string, content: string) {
    const out = getOutputChannel();
    out.clear();
    out.appendLine(`── ${title} ──`);
    out.appendLine('');
    out.appendLine(content);
    out.show(true);
}

async function runAndShow(title: string, args: string[], workingDir?: string) {
    try {
        const { stdout } = await run(args, workingDir);
        showOutput(title, stdout);
    } catch (e: any) {
        vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
        const out = getOutputChannel();
        out.show(true);
    }
}

// ── Commands ─────────────────────────────────────────────────

async function cmdList() {
    await runAndShow('Variables', ['list']);
}

async function cmdValidate(uri?: vscode.Uri) {
    await runAndShow('Schema Validation', ['validate', '--json'], cwdFromUri(uri));
}

async function cmdScan(uri?: vscode.Uri) {
    await runAndShow('Secret Scan', ['scan', '--json'], cwdFromUri(uri));
}

async function cmdProfileSwitch() {
    try {
        const { stdout } = await run(['profile', 'list', '--json']);
        const profiles: string[] = JSON.parse(stdout);

        if (profiles.length === 0) {
            vscode.window.showInformationMessage('No profiles configured.');
            return;
        }

        const selected = await vscode.window.showQuickPick(profiles, {
            placeHolder: 'Select profile to switch to',
        });

        if (selected) {
            await run(['profile', 'switch', selected]);
            vscode.window.showInformationMessage(`Switched to profile: ${selected}`);
        }
    } catch (e: any) {
        vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
    }
}

async function cmdProfileDiff() {
    try {
        const { stdout } = await run(['profile', 'list', '--json']);
        const profiles: string[] = JSON.parse(stdout);

        if (profiles.length < 2) {
            vscode.window.showInformationMessage('Need at least 2 profiles to diff.');
            return;
        }

        const from = await vscode.window.showQuickPick(profiles, { placeHolder: 'From profile' });
        if (!from) return;

        const to = await vscode.window.showQuickPick(
            profiles.filter(p => p !== from),
            { placeHolder: 'To profile' }
        );
        if (!to) return;

        await runAndShow('Profile Diff', ['profile', 'diff', from, to]);
    } catch (e: any) {
        vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
    }
}

async function cmdSchemaGenerate(uri?: vscode.Uri) {
    try {
        const dir = cwdFromUri(uri);
        await run(['schema', 'generate', '--output', '.env.schema'], dir);
        vscode.window.showInformationMessage('Generated .env.schema');
        const doc = await vscode.workspace.openTextDocument(
            vscode.Uri.joinPath(vscode.workspace.workspaceFolders![0].uri, '.env.schema')
        );
        await vscode.window.showTextDocument(doc);
    } catch (e: any) {
        vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
    }
}

async function cmdExport(uri?: vscode.Uri) {
    const formats = ['dotenv', 'json', 'yaml', 'toml', 'docker', 'k8s', 'tfvars'];
    const format = await vscode.window.showQuickPick(formats, {
        placeHolder: 'Export format',
    });
    if (!format) return;

    await runAndShow(`Export (${format})`, ['export', '--format', format], cwdFromUri(uri));
}

async function cmdSyncStatus() {
    await runAndShow('Sync Status', ['sync', 'status']);
}

async function cmdSyncPush() {
    try {
        const { stdout } = await run(['sync', 'push']);
        vscode.window.showInformationMessage('EnvForge: Sync pushed');
        showOutput('Sync Push', stdout);
    } catch (e: any) {
        vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
    }
}

async function cmdSyncPull() {
    try {
        const { stdout } = await run(['sync', 'pull']);
        vscode.window.showInformationMessage('EnvForge: Sync pulled');
        showOutput('Sync Pull', stdout);
    } catch (e: any) {
        vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
    }
}

async function cmdDoctor() {
    await runAndShow('Health Check', ['doctor']);
}

async function cmdCheck(uri?: vscode.Uri) {
    await runAndShow('All Checks', ['check', '--json'], cwdFromUri(uri));
}

async function cmdRestartLsp() {
    // The LSP client doesn't expose restart directly.
    // Reload the window to restart the extension + LSP.
    vscode.commands.executeCommand('workbench.action.reloadWindow');
}
