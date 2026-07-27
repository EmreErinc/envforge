import * as vscode from 'vscode';
import * as cp from 'child_process';
import { getEnvforgePath } from './extension';

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

interface SearchResult {
  key: string;
  value: string;
  source_file?: string;
  line_number?: number;
  score?: number;
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

export class EnvTreeProvider implements vscode.TreeDataProvider<TreeNode> {
  private _onDidChangeTreeData = new vscode.EventEmitter<TreeNode | undefined>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  private groups: GroupNode[] = [];
  private flat: VarNode[] = [];
  private grouped = true;
  private filterQuery: string | undefined;
  private filteredVars: VarNode[] | undefined;
  private _title = 'Variables';

  get title(): string {
    return this._title;
  }

  refresh() {
    this.loadVariables();
  }

  toggleGrouping() {
    this.grouped = !this.grouped;
    this._onDidChangeTreeData.fire(undefined);
  }

  async setFilter(query: string | undefined) {
    if (!query || query.trim() === '') {
      await this.clearFilter();
    } else {
      this.filterQuery = query.trim();
      await this.applyFilter();
      this._title = 'Variables (filtered)';
      this._onDidChangeTreeData.fire(undefined);
    }
  }

  async clearFilter() {
    this.filterQuery = undefined;
    this.filteredVars = undefined;
    this._title = 'Variables';
    this._onDidChangeTreeData.fire(undefined);
  }

  private async applyFilter() {
    const query = this.filterQuery;
    if (!query) {
      this.filteredVars = undefined;
      return;
    }

    const workDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    if (!workDir) {
      this.filteredVars = [];
      return;
    }

    try {
      const results = await new Promise<SearchResult[]>((resolve, reject) => {
        cp.execFile(
          getEnvforgePath(),
          ['search', query, '--json', '--reveal'],
          { cwd: workDir, timeout: 10000 },
          (err, stdout) => {
            if (err || !stdout?.trim()) {
              reject(err || new Error('empty output'));
              return;
            }
            try {
              resolve(JSON.parse(stdout));
            } catch {
              resolve([]);
            }
          },
        );
      });

      if (results.length === 0) {
        this.filteredVars = [];
      } else {
        this.filteredVars = results.map(r => new VarNode({
          key: r.key,
          value: r.value,
          source_file: r.source_file,
          line_number: r.line_number,
        }));
      }
    } catch {
      this.filteredVars = [];
    }
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
      ['list', '--json', '--reveal'],
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

        if (this.filterQuery) {
          this.applyFilter().then(() => {
            this._onDidChangeTreeData.fire(undefined);
          });
        } else {
          this._onDidChangeTreeData.fire(undefined);
        }
      },
    );
  }

  getTreeItem(element: TreeNode): vscode.TreeItem {
    return element;
  }

  getChildren(element?: TreeNode): TreeNode[] {
    if (!getEnvforgePath()) {
      if (!element) {
        const item = new vscode.TreeItem('EnvForge CLI is disabled');
        item.description = 'Click to open install page';
        item.iconPath = new vscode.ThemeIcon('warning');
        item.command = {
          command: 'envforge.showWelcome',
          title: 'Install EnvForge CLI',
        };
        return [item as any];
      }
      return [];
    }

    if (this.filteredVars !== undefined) {
      if (!element) {
        return this.filteredVars;
      }
      return [];
    }

    if (!element) {
      return this.grouped ? this.groups : this.flat;
    }
    if (element instanceof GroupNode) {
      return element.vars;
    }
    return [];
  }
}

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
    if (!getEnvforgePath()) {
      const item = new vscode.TreeItem('EnvForge CLI is disabled');
      item.description = 'Click to open install page';
      item.iconPath = new vscode.ThemeIcon('warning');
      item.command = {
        command: 'envforge.showWelcome',
        title: 'Install EnvForge CLI',
      };
      return [item];
    }
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
      item.contextValue = p.active ? 'envProfileActive' : 'envProfile';

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

function groupByPrefix(vars: EnvVar[]): GroupNode[] {
  const prefixMap = new Map<string, EnvVar[]>();
  const ungrouped: EnvVar[] = [];

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

  groups.sort((a, b) => a.groupName.localeCompare(b.groupName));

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
