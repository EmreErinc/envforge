import * as vscode from 'vscode';
import * as cp from 'child_process';
import { getEnvforgePath } from './extension';

export class StatusBar implements vscode.Disposable {
    private item: vscode.StatusBarItem;
    private timer: NodeJS.Timeout | undefined;

    constructor() {
        this.item = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Left,
            50
        );
        this.item.command = 'envforge.list';
        this.item.tooltip = 'EnvForge — click to list variables';
    }

    update() {
        this.refresh();
        // Refresh every 30s
        this.timer = setInterval(() => this.refresh(), 30000);
    }

    private refresh() {
        const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        if (!cwd) {
            this.item.hide();
            return;
        }

        cp.execFile(
            getEnvforgePath(),
            ['list', '--json'],
            { cwd, timeout: 5000 },
            (err, stdout) => {
                if (err || !stdout) {
                    this.item.text = '$(symbol-variable) envforge';
                    this.item.show();
                    return;
                }

                try {
                    const vars = JSON.parse(stdout);
                    const count = Array.isArray(vars) ? vars.length : 0;
                    this.item.text = `$(symbol-variable) ${count} vars`;
                    this.item.show();
                } catch {
                    this.item.text = '$(symbol-variable) envforge';
                    this.item.show();
                }
            }
        );
    }

    dispose() {
        if (this.timer) {
            clearInterval(this.timer);
        }
        this.item.dispose();
    }
}
