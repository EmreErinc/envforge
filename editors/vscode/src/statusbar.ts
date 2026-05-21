import * as vscode from 'vscode';
import * as cp from 'child_process';
import { getEnvforgePath } from './extension';

export class StatusBar implements vscode.Disposable {
    private varsItem: vscode.StatusBarItem;
    private fenceItem: vscode.StatusBarItem;
    private volatileItem: vscode.StatusBarItem;
    private slowTimer: NodeJS.Timeout | undefined;
    private fastTimer: NodeJS.Timeout | undefined;

    constructor() {
        this.varsItem = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Left,
            50,
        );
        this.varsItem.command = 'envforge.list';
        this.varsItem.tooltip = 'EnvForge — click to list variables';

        this.fenceItem = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Left,
            49,
        );
        this.fenceItem.command = 'envforge.fenceToggle';
        this.fenceItem.tooltip = 'EnvForge fence';

        this.volatileItem = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Left,
            48,
        );
        this.volatileItem.command = 'envforge.volatileExtend';
        this.volatileItem.tooltip = 'EnvForge volatile lease — click to extend';
    }

    update() {
        this.refreshSlow();
        this.refreshVolatile();
        // Vars + fence change rarely; poll every 30 s.
        this.slowTimer = setInterval(() => this.refreshSlow(), 30000);
        // Lease countdown needs sub-minute granularity — refresh every
        // 10 s so the displayed time stays roughly current without
        // flooding the user's machine with subprocess spawns.
        this.fastTimer = setInterval(() => this.refreshVolatile(), 10000);
    }

    private refreshSlow() {
        const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        if (!cwd) {
            this.varsItem.hide();
            this.fenceItem.hide();
            return;
        }
        this.refreshVarsCount(cwd);
        this.refreshFenceState(cwd);
    }

    private refreshVarsCount(cwd: string) {
        cp.execFile(
            getEnvforgePath(),
            ['list', '--json'],
            { cwd, timeout: 5000 },
            (err, stdout) => {
                if (err || !stdout) {
                    this.varsItem.text = '$(symbol-variable) envforge';
                    this.varsItem.show();
                    return;
                }

                try {
                    const vars = JSON.parse(stdout);
                    const count = Array.isArray(vars) ? vars.length : 0;
                    this.varsItem.text = `$(symbol-variable) ${count} vars`;
                    this.varsItem.show();
                } catch {
                    this.varsItem.text = '$(symbol-variable) envforge';
                    this.varsItem.show();
                }
            },
        );
    }

    private refreshFenceState(cwd: string) {
        cp.execFile(
            getEnvforgePath(),
            ['fence', '--status', '--json'],
            { cwd, timeout: 5000 },
            (err, stdout) => {
                if (err || !stdout) {
                    this.fenceItem.hide();
                    return;
                }
                try {
                    const status = JSON.parse(stdout);
                    const allFenced = !!status.all_fenced;
                    if (allFenced) {
                        this.fenceItem.text = '$(shield) AI BLOCKED';
                        this.fenceItem.color = new vscode.ThemeColor(
                            'statusBarItem.warningForeground',
                        );
                        this.fenceItem.backgroundColor = new vscode.ThemeColor(
                            'statusBarItem.warningBackground',
                        );
                        this.fenceItem.tooltip =
                            'EnvForge: AI BLOCKED — all fence files present. Click to re-run fence enable.';
                    } else {
                        this.fenceItem.text = '$(shield) AI ALLOWED';
                        this.fenceItem.color = undefined;
                        this.fenceItem.backgroundColor = undefined;
                        this.fenceItem.tooltip =
                            'EnvForge: AI ALLOWED — fence not active. Click to enable fence.';
                    }
                    this.fenceItem.show();
                } catch {
                    this.fenceItem.hide();
                }
            },
        );
    }

    private refreshVolatile() {
        const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        if (!cwd) {
            this.volatileItem.hide();
            return;
        }
        cp.execFile(
            getEnvforgePath(),
            ['lease', 'list', '--json'],
            { cwd, timeout: 5000 },
            (err, stdout) => {
                if (err || !stdout) {
                    this.volatileItem.hide();
                    return;
                }
                try {
                    const parsed = JSON.parse(stdout);
                    const leases: Array<{
                        name: string;
                        status: string;
                        remaining_seconds: number | null;
                        key_count: number | null;
                    }> = parsed.leases ?? [];
                    const active = leases
                        .filter(
                            l =>
                                l.status === 'active' &&
                                typeof l.remaining_seconds === 'number' &&
                                l.remaining_seconds > 0,
                        )
                        .sort(
                            (a, b) =>
                                (a.remaining_seconds ?? 0) -
                                (b.remaining_seconds ?? 0),
                        )[0];
                    if (!active) {
                        this.volatileItem.hide();
                        return;
                    }
                    const remaining = active.remaining_seconds ?? 0;
                    this.volatileItem.text = `$(clock) volatile: ${formatDuration(remaining)}`;
                    // Pulse the background as the lease nears expiry so
                    // the user notices before the timer hits zero.
                    if (remaining <= 60) {
                        this.volatileItem.backgroundColor =
                            new vscode.ThemeColor('statusBarItem.errorBackground');
                    } else if (remaining <= 300) {
                        this.volatileItem.backgroundColor =
                            new vscode.ThemeColor('statusBarItem.warningBackground');
                    } else {
                        this.volatileItem.backgroundColor = undefined;
                    }
                    const keyCount =
                        active.key_count == null
                            ? 'all keys'
                            : `${active.key_count} key${active.key_count === 1 ? '' : 's'}`;
                    this.volatileItem.tooltip = `EnvForge lease "${active.name}" — ${keyCount}, ${formatDuration(remaining)} remaining.`;
                    this.volatileItem.show();
                } catch {
                    this.volatileItem.hide();
                }
            },
        );
    }

    dispose() {
        if (this.slowTimer) clearInterval(this.slowTimer);
        if (this.fastTimer) clearInterval(this.fastTimer);
        this.varsItem.dispose();
        this.fenceItem.dispose();
        this.volatileItem.dispose();
    }
}

function formatDuration(totalSeconds: number): string {
    const s = Math.max(0, Math.floor(totalSeconds));
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = s % 60;
    if (h > 0) return `${h}h ${m}m`;
    if (m > 0) return `${m}m ${sec}s`;
    return `${sec}s`;
}
