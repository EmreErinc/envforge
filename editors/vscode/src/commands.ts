import * as vscode from 'vscode';
import * as cp from 'child_process';
import { getEnvforgePath, getOutputChannel, getClient } from './extension';
import { StatusBar } from './statusbar';
import { EnvTreeProvider, ProfileTreeProvider } from './treeview';

import { WelcomeWebviewPanel } from './welcome';

export function registerCommands(
  context: vscode.ExtensionContext,
  statusBar: StatusBar,
  treeProvider: EnvTreeProvider,
  profileProvider: ProfileTreeProvider,
) {
  context.subscriptions.push(
    vscode.commands.registerCommand('envforge.showWelcome', () => {
      WelcomeWebviewPanel.show(context);
    })
  );

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
    ['envforge.fenceToggle', () => cmdFenceToggle(statusBar)],
    ['envforge.runVolatile', cmdRunVolatile],
    ['envforge.revealValue', cmdRevealValue],
    ['envforge.canaryScan', cmdCanaryScan],
    ['envforge.canaryCheck', cmdCanaryCheck],
    ['envforge.volatileExtend', () => cmdVolatileExtend(statusBar)],
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

async function cmdFenceToggle(statusBar: StatusBar) {
    const client = getClient();
    if (!client) {
        vscode.window.showWarningMessage('EnvForge LSP not running');
        return;
    }

    // Ask the server which direction we're about to go so we can show
    // an accurate confirmation prompt.
    let currentlyFenced = false;
    try {
        const statusResult: any = await client.sendRequest('envforge/fenceStatus', {});
        currentlyFenced = !!statusResult?.result?.all_fenced;
    } catch {
        // Status probe failed — assume not fenced (enable direction)
        // and let the toggle command itself fail loudly if state is bad.
    }

    const promptText = currentlyFenced
        ? 'Disable EnvForge fence? Removes envforge-owned content from .envforgeignore, .cursorignore, .cursorrules, .github/copilot-instructions.md, .claude/settings.json. User content is preserved.'
        : 'Enable EnvForge fence? Writes .envforgeignore, .cursorignore, .cursorrules, .github/copilot-instructions.md, .claude/settings.json.';
    const confirmButton = currentlyFenced ? 'Disable Fence' : 'Enable Fence';

    const confirm = await vscode.window.showWarningMessage(
        promptText,
        { modal: true },
        confirmButton
    );
    if (confirm !== confirmButton) {
        return;
    }

    try {
        const result: any = await client.sendRequest('envforge/fenceToggle', {});
        const action = result?.result?.action ?? (currentlyFenced ? 'disabled' : 'enabled');
        vscode.window.showInformationMessage(`EnvForge fence ${action}`);
        statusBar.update();
        // Fence flip changes the file-decoration story for every
        // .env* file in the workspace. Refresh all cached badges so
        // the explorer redraws immediately.
        vscode.commands.executeCommand('envforge.decorations.refreshAll');
        // Refresh security view if it exists
        vscode.commands.executeCommand('envforge.refreshSecurity');
    } catch (err: any) {
        vscode.window.showErrorMessage(`Fence toggle failed: ${err?.message || err}`);
    }
}

async function cmdRunVolatile() {
    const client = getClient();
    if (!client) {
        vscode.window.showWarningMessage('EnvForge LSP not running');
        return;
    }

    // Pre-fill with the active editor's selection so a user can mark up
    // a script line and run it inside the wrapper with zero typing.
    const editor = vscode.window.activeTextEditor;
    const selected = editor?.document.getText(editor.selection)?.trim();

    const command = await vscode.window.showInputBox({
        prompt: 'Command to run with volatile envforge session',
        placeHolder: 'npm test',
        value: selected || '',
        validateInput: v => (v.trim() === '' ? 'Command cannot be empty' : null),
    });
    if (!command) return;

    const ttl = await vscode.window.showQuickPick(
        ['5m', '15m', '30m', '1h', '2h', 'Custom…'],
        { placeHolder: 'Session TTL (auto-revokes after this)' },
    );
    if (!ttl) return;
    let ttlFinal = ttl;
    if (ttl === 'Custom…') {
        const v = await vscode.window.showInputBox({
            prompt: 'TTL duration (e.g. 45m, 2h, 1d)',
            value: '30m',
            validateInput: s =>
                /^\d+[smhd]$/i.test(s.trim()) ? null : 'Format: <number><s|m|h|d>',
        });
        if (!v) return;
        ttlFinal = v.trim();
    }

    let response: any;
    try {
        response = await client.sendRequest('envforge/runVolatile', { command, ttl: ttlFinal });
    } catch (err: any) {
        vscode.window.showErrorMessage(`run-volatile failed: ${err?.message || err}`);
        return;
    }
    if (response?.ok !== true) {
        vscode.window.showErrorMessage(`run-volatile failed: ${response?.error || 'unknown'}`);
        return;
    }
    // The server returns a structured descriptor (binary + args) instead of a
    // pre-formed shell string, to avoid injection. Build the terminal line from
    // it: the validated command goes raw after `--` so the terminal shell
    // tokenizes it (e.g. "npm test" → argv [npm, test]).
    const r = response.result;
    const q = (s: string) => (/\s/.test(s) ? `"${s}"` : s);
    const wrapper = `${q(r.binary)} run --volatile ${r.ttl} -- ${r.original_command}`;
    const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    const terminal = vscode.window.createTerminal({
        name: `EnvForge volatile (${ttlFinal})`,
        cwd,
    });
    terminal.show(true);
    terminal.sendText(wrapper);
}

async function cmdRevealValue(arg?: any) {
    const client = getClient();
    if (!client) {
        vscode.window.showWarningMessage('EnvForge LSP not running');
        return;
    }

    let key: string | undefined =
        typeof arg === 'string'
            ? arg
            : arg?.envVar?.key ?? arg?.key ?? undefined;

    if (!key) {
        key = await vscode.window.showInputBox({
            prompt: 'Env var key to reveal',
            placeHolder: 'DB_PASSWORD',
            validateInput: v => (v.trim() === '' ? 'Key cannot be empty' : null),
        });
        if (!key) return;
    }

    const reason = await vscode.window.showInputBox({
        prompt: `Why reveal ${key}? (logged to envforge audit)`,
        placeHolder: 'e.g. debugging staging deploy',
        value: '',
    });
    // Empty reason still proceeds — confirm path below gates the action.
    const confirm = await vscode.window.showWarningMessage(
        `Reveal value of ${key}? This will be logged to the envforge audit stream.`,
        { modal: true },
        'Reveal',
    );
    if (confirm !== 'Reveal') return;

    let response: any;
    try {
        response = await client.sendRequest('envforge/revealValue', { key, reason: reason ?? '' });
    } catch (err: any) {
        vscode.window.showErrorMessage(`reveal failed: ${err?.message || err}`);
        return;
    }
    if (response?.ok !== true) {
        vscode.window.showErrorMessage(`reveal failed: ${response?.error || 'unknown'}`);
        return;
    }
    const value: string = response.result.value ?? '';
    const sourceFile: string = response.result.source_file ?? '';

    // M6: minimize revealed-value residency. The value is shown ONCE in an
    // ephemeral modal and is never written to the OutputChannel / log. Clipboard
    // copy is strictly opt-in and explicitly warned: clipboard managers and OS
    // clipboard-sync can retain a copy beyond our best-effort auto-clear, and JS
    // strings can't be zeroized — so the default path keeps the value off the
    // clipboard entirely, and the copy window is short (15 s).
    const COPY = 'Copy (clipboard may be retained)';
    const choice = await vscode.window.showInformationMessage(
        `${key} = ${value}\n(source: ${sourceFile})\n\nShown once; not logged.`,
        { modal: true },
        COPY,
    );
    if (choice === COPY) {
        await vscode.env.clipboard.writeText(value);
        vscode.window.showWarningMessage(
            'Value copied. Clipboard auto-clears in 15s — but clipboard managers or clipboard sync may keep a copy.',
        );
        setTimeout(async () => {
            const current = await vscode.env.clipboard.readText();
            if (current === value) {
                await vscode.env.clipboard.writeText('');
            }
        }, 15000);
    }
}

async function cmdCanaryScan() {
    const client = getClient();
    if (!client) {
        vscode.window.showWarningMessage('EnvForge LSP not running');
        return;
    }

    // Two entry paths — paste text or pick a file. Editor selection
    // wins as the third path: if the user has text highlighted, that's
    // almost always what they want to scan.
    const editor = vscode.window.activeTextEditor;
    const selected = editor?.document.getText(editor.selection);

    let mode: 'text' | 'file' | undefined;
    if (selected && selected.trim().length > 0) {
        mode = 'text';
    } else {
        const choice = await vscode.window.showQuickPick(
            ['Paste text to scan…', 'Pick a file to scan…'],
            { placeHolder: 'How do you want to scan for canary tokens?' },
        );
        if (!choice) return;
        mode = choice.startsWith('Paste') ? 'text' : 'file';
    }

    const args: Record<string, string> = {};
    if (mode === 'text') {
        let text = selected;
        if (!text) {
            text = await vscode.window.showInputBox({
                prompt: 'Paste text (log line, stack trace, diff) to scan',
                placeHolder: 'cnry_…',
                validateInput: v => (v.trim() === '' ? 'Text cannot be empty' : null),
            });
            if (!text) return;
        }
        args.text = text;
    } else {
        const picked = await vscode.window.showOpenDialog({
            canSelectFiles: true,
            canSelectFolders: false,
            canSelectMany: false,
            openLabel: 'Scan for canary tokens',
        });
        if (!picked || picked.length === 0) return;
        args.file = picked[0].fsPath;
    }

    let response: any;
    try {
        response = await client.sendRequest('envforge/canaryScan', args);
    } catch (err: any) {
        vscode.window.showErrorMessage(`canary.scan failed: ${err?.message || err}`);
        return;
    }
    if (response?.ok !== true) {
        vscode.window.showErrorMessage(`canary.scan failed: ${response?.error || 'unknown'}`);
        return;
    }

    const count: number = response.result.match_count ?? 0;
    const matches: Array<{ token: string; byte_offset: number | null; line_number: number | null }> =
        response.result.matches ?? [];

    if (count === 0) {
        vscode.window.showInformationMessage(
            'EnvForge canary scan: no registered tripwire tokens found.',
        );
        return;
    }

    // Route detailed match output to the channel so the user can copy
    // line numbers, but show a summary banner first.
    const out = getOutputChannel();
    out.clear();
    out.appendLine(`── EnvForge canary scan ──`);
    out.appendLine(`${count} match${count === 1 ? '' : 'es'} found:`);
    out.appendLine('');
    for (const m of matches) {
        const loc = m.line_number != null
            ? `line ${m.line_number}`
            : m.byte_offset != null
                ? `byte ${m.byte_offset}`
                : '?';
        out.appendLine(`  ${loc}: ${m.token}`);
    }
    out.show(true);
    vscode.window.showWarningMessage(
        `EnvForge: ${count} canary token${count === 1 ? '' : 's'} detected — see output for details.`,
    );
}

async function cmdCanaryCheck() {
    const client = getClient();
    if (!client) {
        vscode.window.showWarningMessage('EnvForge LSP not running');
        return;
    }
    let response: any;
    try {
        response = await client.sendRequest('envforge/canaryCheck', {});
    } catch (err: any) {
        vscode.window.showErrorMessage(`canary.check failed: ${err?.message || err}`);
        return;
    }
    if (response?.ok !== true) {
        vscode.window.showErrorMessage(`canary.check failed: ${response?.error || 'unknown'}`);
        return;
    }

    const count: number = response.result.triggered_count ?? 0;
    const triggered: Array<{ key: string; pattern: string; trigger_count: number; created_at: string }> =
        response.result.triggered ?? [];

    if (count === 0) {
        vscode.window.showInformationMessage(
            'EnvForge canary check: no triggered tripwires. All quiet.',
        );
        return;
    }
    const out = getOutputChannel();
    out.clear();
    out.appendLine(`── EnvForge triggered canaries ──`);
    out.appendLine(`${count} tripwire${count === 1 ? '' : 's'} triggered:`);
    out.appendLine('');
    for (const c of triggered) {
        out.appendLine(`  ${c.key} (${c.pattern}) — ${c.trigger_count} hit${c.trigger_count === 1 ? '' : 's'}, created ${c.created_at}`);
    }
    out.show(true);
    vscode.window.showErrorMessage(
        `EnvForge: ${count} triggered canary${count === 1 ? '' : 's'}. Review immediately.`,
    );
}

async function cmdVolatileExtend(statusBar: StatusBar) {
    const client = getClient();
    if (!client) {
        vscode.window.showWarningMessage('EnvForge LSP not running');
        return;
    }

    // Resolve the lease to extend: query the active one. If none active,
    // tell the user instead of silently doing nothing.
    let statusResp: any;
    try {
        statusResp = await client.sendRequest('envforge/volatileStatus', {});
    } catch (err: any) {
        vscode.window.showErrorMessage(`volatile.status failed: ${err?.message || err}`);
        return;
    }
    const active = statusResp?.result;
    if (!active || active === null) {
        vscode.window.showInformationMessage('No active volatile lease to extend.');
        return;
    }
    const name: string = active.name;

    const ttlChoice = await vscode.window.showQuickPick(
        ['5m', '15m', '30m', '1h', '2h', 'Custom…'],
        { placeHolder: `Extend lease "${name}" — new TTL (replaces remaining time)` },
    );
    if (!ttlChoice) return;
    let ttl = ttlChoice;
    if (ttlChoice === 'Custom…') {
        const v = await vscode.window.showInputBox({
            prompt: 'TTL duration (e.g. 45m, 2h, 1d)',
            value: '30m',
            validateInput: s =>
                /^\d+[smhd]$/i.test(s.trim()) ? null : 'Format: <number><s|m|h|d>',
        });
        if (!v) return;
        ttl = v.trim();
    }

    try {
        const resp: any = await client.sendRequest('envforge/volatileExtend', { name, ttl });
        if (resp?.ok !== true) {
            vscode.window.showErrorMessage(`Extend failed: ${resp?.error || 'unknown'}`);
            return;
        }
        vscode.window.showInformationMessage(
            `Lease "${name}" extended by ${ttl}.`,
        );
        statusBar.update();
    } catch (err: any) {
        vscode.window.showErrorMessage(`Extend failed: ${err?.message || err}`);
    }
}
