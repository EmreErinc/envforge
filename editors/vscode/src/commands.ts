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

  const commands: [string, () => Promise<void>][] = [
    ['envforge.list', cmdList],
    ['envforge.profileSwitch', cmdProfileSwitch],
    ['envforge.profileDiff', cmdProfileDiff],
    ['envforge.syncStatus', cmdSyncStatus],
    ['envforge.syncPush', cmdSyncPush],
    ['envforge.syncPull', cmdSyncPull],
    ['envforge.doctor', cmdDoctor],
    ['envforge.restartLsp', cmdRestartLsp],
    ['envforge.runWizard', cmdRunWizard],
    ['envforge.projectInit', cmdProjectInit],
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
    vscode.commands.registerCommand('envforge.clearSearch', () => {
      treeProvider.clearFilter();
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
        const { stdout } = await run(['profile', 'list', '--json']);
        const profiles: string[] = JSON.parse(stdout);
        if (item.contextValue === 'envProfileActive') {
          const other = await vscode.window.showQuickPick(
            profiles.filter(p => p !== name),
            { placeHolder: 'Diff with profile' }
          );
          if (!other) return;
          await runAndShow('Profile Diff', ['profile', 'diff', name, other]);
        } else {
          await runAndShow('Profile Diff', ['profile', 'diff', name]);
        }
      } catch (e: any) {
        vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
      }
    }),
    vscode.commands.registerCommand('envforge.profileOpenFile', async (item: vscode.TreeItem) => {
      const name = typeof item?.label === 'string' ? item.label : (item?.label as any)?.label;
      if (!name) return;
      const wsFolder = vscode.workspace.workspaceFolders?.[0];
      if (!wsFolder) return;
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
    // Search command
    vscode.commands.registerCommand('envforge.searchVariables', async () => {
      await vscode.commands.executeCommand('envforgeVariables.focus');
      const input = vscode.window.createInputBox();
      input.title = 'Search Environment Variables';
      input.placeholder = 'Type to search variables...';
      input.prompt = 'Fuzzy search via envforge search. Press Enter to filter, Escape to clear.';
      input.show();

      let accepted = false;
      input.onDidAccept(() => {
        accepted = true;
        const query = input.value.trim();
        input.hide();
        treeProvider.setFilter(query || undefined);
      });

      input.onDidHide(() => {
        if (!accepted) {
          treeProvider.setFilter(undefined);
        }
      });
    }),
    // Add variable command
    vscode.commands.registerCommand('envforge.addVariable', async () => {
      const key = await vscode.window.showInputBox({
        prompt: 'Variable key (name)',
        placeHolder: 'MY_VAR',
        validateInput: v => v.trim() === '' ? 'Key cannot be empty' : null,
      });
      if (!key) return;

      const value = await vscode.window.showInputBox({
        prompt: `Value for ${key}`,
        placeHolder: 'my-value',
      });
      if (value === undefined) return;

      try {
        await run(['set', key, value]);
        vscode.window.showInformationMessage(`Added: ${key}`);
        treeProvider.refresh();
        statusBar.update();
      } catch (e: any) {
        vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
      }
    }),
    // Edit value command
    vscode.commands.registerCommand('envforge.editValue', async (arg: any) => {
      const key = arg?.envVar?.key ?? arg?.key;
      const currentValue = arg?.envVar?.value ?? arg?.value;
      if (!key) return;

      const isRedacted = typeof currentValue === 'string' && currentValue.includes('***');
      const placeholder = isRedacted ? '[REDACTED] — enter new value' : currentValue;

      const newValue = await vscode.window.showInputBox({
        prompt: `Edit value for ${key}`,
        value: isRedacted ? '' : currentValue,
        placeHolder: placeholder,
      });
      if (newValue === undefined) return;

      try {
        await run(['set', key, newValue]);
        vscode.window.showInformationMessage(`Updated: ${key}`);
        treeProvider.refresh();
        statusBar.update();
      } catch (e: any) {
        vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
      }
    }),
    // Delete variable command
    vscode.commands.registerCommand('envforge.deleteVariable', async (arg: any) => {
      const key = arg?.envVar?.key ?? arg?.key;
      if (!key) return;

      const confirm = await vscode.window.showWarningMessage(
        `Delete variable "${key}"? This modifies your shell configuration file.`,
        { modal: true },
        'Delete',
      );
      if (confirm !== 'Delete') return;

      try {
        await run(['delete', key]);
        vscode.window.showInformationMessage(`Deleted: ${key}`);
        treeProvider.refresh();
        statusBar.update();
      } catch (e: any) {
        vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
      }
    }),
    // Rename variable command
    vscode.commands.registerCommand('envforge.renameVariable', async (arg: any) => {
      const oldKey = arg?.envVar?.key ?? arg?.key;
      if (!oldKey) return;

      const newKey = await vscode.window.showInputBox({
        prompt: `Rename "${oldKey}" to`,
        value: oldKey,
        validateInput: v => v.trim() === '' ? 'Key cannot be empty' : null,
      });
      if (!newKey || newKey === oldKey) return;

      try {
        await run(['move', oldKey, newKey]);
        vscode.window.showInformationMessage(`Renamed: ${oldKey} → ${newKey}`);
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
        await run(['schema', 'generate', '--output', '.env.schema.toml'], dir);
        vscode.window.showInformationMessage('Generated .env.schema.toml');
        const doc = await vscode.workspace.openTextDocument(
            vscode.Uri.joinPath(vscode.workspace.workspaceFolders![0].uri, '.env.schema.toml')
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

async function cmdRunWizard() {
    await launchInTerminal('EnvForge Wizard', ['project', 'wizard']);
}

async function cmdProjectInit() {
    await launchInTerminal('EnvForge Project Init', ['project', 'init']);
}

async function launchInTerminal(name: string, args: string[]) {
    const bin = getEnvforgePath();
    const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    const terminal = vscode.window.createTerminal({ name, cwd });
    terminal.show(true);
    const quoted = args.map(a => /\s/.test(a) ? `"${a}"` : a).join(' ');
    terminal.sendText(`${bin} ${quoted}`);
}
