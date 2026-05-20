import * as vscode from 'vscode';
import * as cp from 'child_process';
import { getEnvforgePath } from './extension';

interface ExposureEntry {
    line: number;
    key: string;
    level: 'red' | 'amber' | 'green';
    reason: string;
    canary?: boolean;
}

interface ExposureMapResponse {
    entries: ExposureEntry[];
    fence_active: boolean;
}

/**
 * Badges `.env*` files in the explorer / open-tabs UI with a single
 * character + tooltip describing the current security state. Pulls the
 * data from the same `envforge exposure --file PATH --json` CLI that
 * powers the in-editor gutter heatmap (P5), so the badge is always
 * consistent with what the user sees inside the file.
 *
 * Badge selection precedence (first match wins):
 *  - 🛡  fence is active for the workspace
 *  - !   any line classified `red`
 *  - ?   any line classified `amber`
 *  - ✓   only `green` lines
 *  - (no badge)  classification failed or file is empty
 *
 * Cache: per-URI result lives in `this.cache` so repeated explorer
 * queries don't fan out to subprocess calls. `refresh(uri)` invalidates
 * and fires `_onDidChange` so the explorer re-asks.
 */
export class EnvFileDecorationProvider
    implements vscode.FileDecorationProvider, vscode.Disposable
{
    private readonly _onDidChange = new vscode.EventEmitter<
        vscode.Uri | vscode.Uri[] | undefined
    >();
    readonly onDidChangeFileDecorations = this._onDidChange.event;

    private cache = new Map<string, vscode.FileDecoration | undefined>();
    private inFlight = new Set<string>();

    provideFileDecoration(uri: vscode.Uri): vscode.FileDecoration | undefined {
        if (uri.scheme !== 'file') return undefined;
        if (!isEnvFile(uri)) return undefined;

        const key = uri.toString();
        if (this.cache.has(key)) {
            return this.cache.get(key);
        }
        if (!this.inFlight.has(key)) {
            this.inFlight.add(key);
            void this.fetch(uri).finally(() => this.inFlight.delete(key));
        }
        return undefined;
    }

    /** Force-refresh one URI (or every cached URI if omitted). */
    refresh(uri?: vscode.Uri) {
        if (uri) {
            this.cache.delete(uri.toString());
            this._onDidChange.fire(uri);
        } else {
            const all = [...this.cache.keys()].map(k => vscode.Uri.parse(k));
            this.cache.clear();
            this._onDidChange.fire(all);
        }
    }

    private async fetch(uri: vscode.Uri) {
        const cwd =
            vscode.workspace.getWorkspaceFolder(uri)?.uri.fsPath ??
            vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        if (!cwd) {
            this.cache.set(uri.toString(), undefined);
            this._onDidChange.fire(uri);
            return;
        }
        const out = await runExposure(uri.fsPath, cwd);
        const decoration = out ? toDecoration(out) : undefined;
        this.cache.set(uri.toString(), decoration);
        this._onDidChange.fire(uri);
    }

    dispose() {
        this._onDidChange.dispose();
    }
}

function isEnvFile(uri: vscode.Uri): boolean {
    const fname = uri.path.split('/').pop() ?? '';
    return (
        fname === '.env' ||
        fname.startsWith('.env.') ||
        (fname.endsWith('.env') && fname !== '.env.schema' && fname !== '.env.schema.toml') ||
        fname === 'env'
    );
}

function runExposure(
    file: string,
    cwd: string,
): Promise<ExposureMapResponse | undefined> {
    return new Promise(resolve => {
        cp.execFile(
            getEnvforgePath(),
            ['exposure', '--file', file],
            { cwd, timeout: 5000 },
            (err, stdout) => {
                if (err || !stdout) {
                    resolve(undefined);
                    return;
                }
                try {
                    resolve(JSON.parse(stdout));
                } catch {
                    resolve(undefined);
                }
            },
        );
    });
}

function toDecoration(map: ExposureMapResponse): vscode.FileDecoration | undefined {
    if (map.fence_active) {
        const d = new vscode.FileDecoration(
            '🛡',
            'EnvForge: fence active — AI agents instructed to refuse reads.',
            new vscode.ThemeColor('charts.green'),
        );
        d.propagate = false;
        return d;
    }
    const hasRed = map.entries.some(e => e.level === 'red');
    const hasAmber = map.entries.some(e => e.level === 'amber');
    if (hasRed) {
        const d = new vscode.FileDecoration(
            '!',
            'EnvForge: plaintext secrets readable by AI agents. Click the file to review.',
            new vscode.ThemeColor('charts.red'),
        );
        d.propagate = false;
        return d;
    }
    if (hasAmber) {
        const d = new vscode.FileDecoration(
            '?',
            'EnvForge: sensitive values present. AI-guard will redact in tool inputs; plaintext lives on disk.',
            new vscode.ThemeColor('charts.yellow'),
        );
        d.propagate = false;
        return d;
    }
    if (map.entries.length > 0) {
        const d = new vscode.FileDecoration(
            '✓',
            'EnvForge: no plaintext secrets detected on this file.',
            new vscode.ThemeColor('charts.green'),
        );
        d.propagate = false;
        return d;
    }
    return undefined;
}
