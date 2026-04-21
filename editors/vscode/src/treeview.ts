import * as vscode from 'vscode';
import * as cp from 'child_process';
import { getEnvforgePath } from './extension';

// ── Types ────────────────────────────────────────────────────

interface EnvVar {
    key: string;
    value: string;
    source_file?: string;
    line_number?: number;
    location?: string;
}

interface ProfileInfo {
    name: string;
    file: string;
    active: boolean;
}

type TreeNode = GroupNode | VarNode;

class GroupNode extends vscode.TreeItem {
    constructor(
        public readonly groupName: string,
        public readonly vars: VarNode[],
    ) {
        super(groupName, vscode.TreeItemCollapsibleState.Collapsed);
        this.description = `${vars.length}`;
        this.iconPath = new vscode.ThemeIcon('folder');
        this.contextValue = 'envGroup';
    }
}

class VarNode extends vscode.TreeItem {
    constructor(public readonly envVar: EnvVar) {
        super(envVar.key, vscode.TreeItemCollapsibleState.None);
        const masked = maskValue(envVar.key, envVar.value);
        this.description = masked;
        this.tooltip = new vscode.MarkdownString(
            `**${envVar.key}**\n\nValue: \`${envVar.value}\`\n\nSource: \`${envVar.source_file || 'unknown'}\`${envVar.line_number ? ` (line ${envVar.line_number})` : ''}`
        );
        this.iconPath = new vscode.ThemeIcon(isSensitive(envVar.key) ? 'lock' : 'symbol-variable');
        this.contextValue = 'envVariable';
        this.command = {
            command: 'envforge.copyKey',
            title: 'Copy Key Name',
            arguments: [envVar.key],
        };
    }
}

// ── Tree Provider ────────────────────────────────────────────

export class EnvTreeProvider implements vscode.TreeDataProvider<TreeNode> {
    private _onDidChangeTreeData = new vscode.EventEmitter<TreeNode | undefined>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

    private groups: GroupNode[] = [];
    private flat: VarNode[] = [];
    private grouped = true;

    refresh() {
        this.loadVariables();
    }

    toggleGrouping() {
        this.grouped = !this.grouped;
        this._onDidChangeTreeData.fire(undefined);
    }

    private loadVariables() {
        const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        if (!cwd) {
            this.groups = [];
            this.flat = [];
            this._onDidChangeTreeData.fire(undefined);
            return;
        }

        cp.execFile(
            getEnvforgePath(),
            ['list', '--json'],
            { cwd, timeout: 10000 },
            (err, stdout) => {
                if (err || !stdout?.trim()) {
                    this.groups = [];
                    this.flat = [];
                    this._onDidChangeTreeData.fire(undefined);
                    return;
                }

                try {
                    const vars: EnvVar[] = JSON.parse(stdout);
                    this.flat = vars.map(v => new VarNode(v));
                    this.groups = groupByPrefix(vars);
                } catch {
                    this.groups = [];
                    this.flat = [];
                }

                this._onDidChangeTreeData.fire(undefined);
            },
        );
    }

    getTreeItem(element: TreeNode): vscode.TreeItem {
        return element;
    }

    getChildren(element?: TreeNode): TreeNode[] {
        if (!element) {
            return this.grouped ? this.groups : this.flat;
        }
        if (element instanceof GroupNode) {
            return element.vars;
        }
        return [];
    }
}

// ── Profile Provider ─────────────────────────────────────────

export class ProfileTreeProvider implements vscode.TreeDataProvider<vscode.TreeItem> {
    private _onDidChangeTreeData = new vscode.EventEmitter<vscode.TreeItem | undefined>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

    private profiles: ProfileInfo[] = [];

    refresh() {
        this.loadProfiles();
    }

    private loadProfiles() {
        const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        if (!cwd) {
            this.profiles = [];
            this._onDidChangeTreeData.fire(undefined);
            return;
        }

        cp.execFile(
            getEnvforgePath(),
            ['profile', 'list'],
            { cwd, timeout: 5000 },
            (err, stdout) => {
                if (err || !stdout) {
                    this.profiles = [];
                    this._onDidChangeTreeData.fire(undefined);
                    return;
                }

                // Parse text output: "  name (file) ← active"
                this.profiles = [];
                for (const line of stdout.split('\n')) {
                    const match = line.match(/^\s+(\S+)\s+\(([^)]+)\)(.*)/);
                    if (match) {
                        this.profiles.push({
                            name: match[1],
                            file: match[2],
                            active: match[3].includes('active'),
                        });
                    }
                }

                this._onDidChangeTreeData.fire(undefined);
            },
        );
    }

    getTreeItem(element: vscode.TreeItem): vscode.TreeItem {
        return element;
    }

    getChildren(): vscode.TreeItem[] {
        if (this.profiles.length === 0) {
            const item = new vscode.TreeItem('No profiles');
            item.description = 'Run: envforge profile create <name>';
            item.iconPath = new vscode.ThemeIcon('info');
            return [item];
        }

        return this.profiles.map(p => {
            const item = new vscode.TreeItem(p.name);
            item.description = p.active ? 'active' : p.file;
            item.iconPath = new vscode.ThemeIcon(p.active ? 'check' : 'circle-outline');
            item.contextValue = p.active ? 'activeProfile' : 'inactiveProfile';

            if (!p.active) {
                item.command = {
                    command: 'envforge.switchToProfile',
                    title: 'Switch Profile',
                    arguments: [p.name],
                };
            }

            return item;
        });
    }
}

// ── Helpers ──────────────────────────────────────────────────

function groupByPrefix(vars: EnvVar[]): GroupNode[] {
    const prefixMap = new Map<string, EnvVar[]>();
    const ungrouped: EnvVar[] = [];

    // Extract prefix (first segment before _)
    for (const v of vars) {
        const parts = v.key.split('_');
        if (parts.length >= 2) {
            const prefix = parts[0] + '_';
            if (!prefixMap.has(prefix)) {
                prefixMap.set(prefix, []);
            }
            prefixMap.get(prefix)!.push(v);
        } else {
            ungrouped.push(v);
        }
    }

    const groups: GroupNode[] = [];

    // Only create groups with 2+ entries
    for (const [prefix, entries] of prefixMap) {
        if (entries.length >= 2) {
            groups.push(new GroupNode(
                `${prefix}*`,
                entries.map(e => new VarNode(e)),
            ));
        } else {
            ungrouped.push(...entries);
        }
    }

    // Sort groups alphabetically
    groups.sort((a, b) => a.groupName.localeCompare(b.groupName));

    // Add "Other" group
    if (ungrouped.length > 0) {
        groups.push(new GroupNode(
            'Other',
            ungrouped.map(e => new VarNode(e)),
        ));
    }

    return groups;
}

function isSensitive(key: string): boolean {
    const upper = key.toUpperCase();
    return ['SECRET', 'PASSWORD', 'TOKEN', 'KEY', 'PRIVATE', 'CREDENTIAL', 'AUTH']
        .some(p => upper.includes(p));
}

function maskValue(key: string, value: string): string {
    if (isSensitive(key) && value.length > 4) {
        return value.substring(0, 3) + '***';
    }
    if (value.length > 40) {
        return value.substring(0, 35) + '...';
    }
    return value;
}
