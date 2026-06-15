import * as vscode from 'vscode';
import * as cp from 'child_process';
import { getEnvforgePath } from './extension';

type SecurityNode = SecurityCategoryNode | SecurityDetailNode;

class SecurityCategoryNode extends vscode.TreeItem {
  constructor(
    public readonly id: string,
    label: string,
    collapsible: vscode.TreeItemCollapsibleState,
    icon: string,
    description?: string,
  ) {
    super(label, collapsible);
    this.iconPath = new vscode.ThemeIcon(icon);
    this.contextValue = `security${id}`;
    if (description) {
      this.description = description;
    }
  }
}

class SecurityDetailNode extends vscode.TreeItem {
  constructor(
    label: string,
    description: string,
    icon: string,
    contextValue?: string,
  ) {
    super(label, vscode.TreeItemCollapsibleState.None);
    this.description = description;
    this.iconPath = new vscode.ThemeIcon(icon);
    if (contextValue) {
      this.contextValue = contextValue;
    }
  }
}

export class SecurityTreeProvider implements vscode.TreeDataProvider<SecurityNode> {
  private _onDidChangeTreeData = new vscode.EventEmitter<SecurityNode | undefined>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  private fenceStatus: any = null;
  private guardStatus: any = null;
  private mcpScanResult: any = null;
  private canaryTokens: any[] = [];
  private loadError: string | null = null;

  refresh() {
    this.loadAll();
  }

  private loadAll() {
    const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    if (!cwd) {
      this.loadError = 'No workspace folder';
      this._onDidChangeTreeData.fire(undefined);
      return;
    }

    this.loadError = null;
    let pending = 4;

    const done = () => {
      pending--;
      if (pending === 0) {
        this._onDidChangeTreeData.fire(undefined);
      }
    };

    this.execJson(['fence', '--status', '--json'], cwd).then(data => {
      this.fenceStatus = data;
      done();
    }).catch(() => { this.fenceStatus = null; done(); });

    this.execJson(['ai-hook', 'status', '--json'], cwd).then(data => {
      this.guardStatus = data;
      done();
    }).catch(() => { this.guardStatus = null; done(); });

    this.execJson(['mcp', 'status', '--json'], cwd).then(data => {
      this.mcpScanResult = data;
      done();
    }).catch(() => { this.mcpScanResult = null; done(); });

    this.execJson(['canary', 'list', '--json'], cwd).then(data => {
      this.canaryTokens = Array.isArray(data) ? data : [];
      done();
    }).catch(() => { this.canaryTokens = []; done(); });
  }

  private execJson(args: string[], cwd: string): Promise<any> {
    return new Promise((resolve, reject) => {
      cp.execFile(getEnvforgePath(), args, { cwd, timeout: 10000 }, (err, stdout) => {
        if (err || !stdout?.trim()) {
          reject(err || new Error('empty'));
          return;
        }
        try {
          resolve(JSON.parse(stdout));
        } catch {
          reject(new Error('parse error'));
        }
      });
    });
  }

  getTreeItem(element: SecurityNode): vscode.TreeItem {
    return element;
  }

  getChildren(element?: SecurityNode): SecurityNode[] {
    if (this.loadError) {
      if (!element) {
        return [new SecurityDetailNode('Error', this.loadError, 'error')];
      }
      return [];
    }

    if (!element) {
      return [
        new SecurityCategoryNode('Fence', 'Fence', vscode.TreeItemCollapsibleState.Collapsed,
          this.fenceStatus?.active ? 'shield' : 'warning',
          this.fenceStatus?.active ? 'Active' : 'Inactive'),
        new SecurityCategoryNode('Guard', 'Guard', vscode.TreeItemCollapsibleState.Collapsed,
          this.guardStatus?.enabled ? 'shield' : 'warning',
          this.guardStatus?.enabled ? 'Enabled' : 'Disabled'),
        new SecurityCategoryNode('Lifecycle', 'Lifecycle', vscode.TreeItemCollapsibleState.Collapsed,
          'calendar', 'Governance'),
        new SecurityCategoryNode('Analytics', 'Analytics', vscode.TreeItemCollapsibleState.Collapsed,
          'graph', 'Usage'),
        new SecurityCategoryNode('MCP', 'MCP Scan', vscode.TreeItemCollapsibleState.Collapsed,
          this.mcpScanResult?.issues?.length > 0 ? 'error' : 'check',
          this.mcpScanResult ? `${this.mcpScanResult.servers?.length || 0} servers` : 'Not run'),
        new SecurityCategoryNode('Canary', 'Canary Tokens', vscode.TreeItemCollapsibleState.Collapsed,
          this.canaryTokens.length > 0 ? 'eye' : 'eye-closed',
          `${this.canaryTokens.length} tokens`),
      ];
    }

    if (element instanceof SecurityCategoryNode) {
      switch (element.id) {
        case 'Fence': return this.fenceChildren();
        case 'Guard': return this.guardChildren();
        case 'Lifecycle': return this.lifecycleChildren();
        case 'Analytics': return this.analyticsChildren();
        case 'MCP': return this.mcpChildren();
        case 'Canary': return this.canaryChildren();
      }
    }

    return [];
  }

  private fenceChildren(): SecurityDetailNode[] {
    if (!this.fenceStatus) {
      return [new SecurityDetailNode('Status', 'Unable to load', 'error')];
    }
    const items: SecurityDetailNode[] = [];
    items.push(new SecurityDetailNode('Active', this.fenceStatus.active ? 'Yes' : 'No',
      this.fenceStatus.active ? 'check' : 'warning'));
    if (this.fenceStatus.files?.length > 0) {
      for (const f of this.fenceStatus.files.slice(0, 20)) {
        items.push(new SecurityDetailNode('Fenced', f, 'file'));
      }
    }
    return items;
  }

  private guardChildren(): SecurityDetailNode[] {
    if (!this.guardStatus) {
      return [new SecurityDetailNode('Status', 'Unable to load', 'error')];
    }
    const items: SecurityDetailNode[] = [];
    items.push(new SecurityDetailNode('Enabled', this.guardStatus.enabled ? 'Yes' : 'No',
      this.guardStatus.enabled ? 'check' : 'warning'));
    if (this.guardStatus.lastCheck) {
      items.push(new SecurityDetailNode('Last Check', this.guardStatus.lastCheck, 'clock'));
    }
    if (this.guardStatus.alertCount !== undefined) {
      items.push(new SecurityDetailNode('Alerts', String(this.guardStatus.alertCount),
        this.guardStatus.alertCount > 0 ? 'alert' : 'check'));
    }
    return items;
  }

  private lifecycleChildren(): SecurityDetailNode[] {
    return [
      new SecurityDetailNode('Run Lifecycle Check', 'Evaluate rules', 'play', 'runLifecycleCheck'),
      new SecurityDetailNode('Manage Rules', 'Lifecycle rule list', 'list-unordered', 'manageLifecycleRules'),
      new SecurityDetailNode('Audit Trail', 'View sync & access history', 'history', 'viewAuditTrail'),
    ];
  }

  private analyticsChildren(): SecurityDetailNode[] {
    return [
      new SecurityDetailNode('Show Unused Secrets', 'Dormant for 90 days', 'trash', 'showUnusedSecrets'),
      new SecurityDetailNode('Usage Summary', 'Event & secret counts', 'pie-chart', 'showUsageSummary'),
      new SecurityDetailNode('Monitor Stream', 'Real-time access events', 'pulse', 'monitorStream'),
    ];
  }

  private mcpChildren(): SecurityDetailNode[] {
    if (!this.mcpScanResult) {
      return [new SecurityDetailNode('Status', 'Not yet scanned', 'info')];
    }
    const items: SecurityDetailNode[] = [];
    const servers = this.mcpScanResult.servers || [];
    const issues = this.mcpScanResult.issues || [];
    items.push(new SecurityDetailNode('Servers Scanned', String(servers.length), 'server'));
    items.push(new SecurityDetailNode('Issues Found', String(issues.length),
      issues.length > 0 ? 'error' : 'check'));
    if (this.mcpScanResult.riskLevel) {
      items.push(new SecurityDetailNode('Risk Level', this.mcpScanResult.riskLevel,
        this.mcpScanResult.riskLevel === 'high' ? 'error' :
        this.mcpScanResult.riskLevel === 'medium' ? 'warning' : 'check'));
    }
    for (const issue of issues.slice(0, 10)) {
      const desc = typeof issue === 'string' ? issue : issue.message || issue.description || JSON.stringify(issue);
      items.push(new SecurityDetailNode('Issue', desc, 'error'));
    }
    return items;
  }

  private canaryChildren(): SecurityDetailNode[] {
    if (this.canaryTokens.length === 0) {
      return [new SecurityDetailNode('No canary tokens', 'Add one to detect exfiltration', 'info')];
    }
    return this.canaryTokens.slice(0, 50).map(t => {
      const triggered = t.triggered || false;
      return new SecurityDetailNode(
        t.key || t.name || 'unknown',
        triggered ? 'TRIGGERED' : 'Clean',
        triggered ? 'alert' : 'check',
        triggered ? 'canaryTriggered' : 'canaryEntry',
      );
    });
  }
}

export function registerSecurityCommands(
  context: vscode.ExtensionContext,
  securityProvider: SecurityTreeProvider,
  treeProvider: import('./treeview').EnvTreeProvider,
  statusBar: import('./statusbar').StatusBar,
) {
  context.subscriptions.push(
    vscode.commands.registerCommand('envforge.refreshSecurity', () => {
      securityProvider.refresh();
    }),
    vscode.commands.registerCommand('envforge.toggleFence', async () => {
      await vscode.commands.executeCommand('envforge.fenceToggle');
    }),
    vscode.commands.registerCommand('envforge.toggleGuard', async () => {
      if (!securityProvider['guardStatus']) {
        vscode.window.showErrorMessage('EnvForge: Unable to determine guard status');
        return;
      }

      const isInstalled = securityProvider['guardStatus'].enabled;
      const tools = securityProvider['guardStatus'].tools || [];
      const installedTools = tools.filter((t: any) => t.installed).map((t: any) => t.name);

      if (isInstalled) {
        const tool = await vscode.window.showQuickPick(['All'].concat(installedTools), {
          placeHolder: 'Select AI tool to remove hooks from'
        });
        if (!tool) return;

        const toolsToRemove = tool === 'All' ? ['claude-code', 'cursor'] : [tool.toLowerCase().replace(' ', '-')];
        for (const t of toolsToRemove) {
          await cliRun(['ai-hook', 'remove', t]);
        }
        vscode.window.showInformationMessage(`Guard hooks removed`);
      } else {
        const tool = await vscode.window.showQuickPick(['Claude Code', 'Cursor', 'Both'], {
          placeHolder: 'Select AI tool to install hooks for'
        });
        if (!tool) return;

        const toolsToInstall = tool === 'Both' ? ['claude-code', 'cursor'] : [tool.toLowerCase().replace(' ', '-')];
        for (const t of toolsToInstall) {
          await cliRun(['ai-hook', 'install', t]);
        }
        vscode.window.showInformationMessage(`Guard hooks installed`);
      }
      securityProvider.refresh();
    }),
    vscode.commands.registerCommand('envforge.runMcpScan', async () => {
      try {
        await cliRun(['mcp', 'harden']);
        vscode.window.showInformationMessage('MCP hardening complete');
        securityProvider.refresh();
      } catch (e: any) {
        vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
      }
    }),
    vscode.commands.registerCommand('envforge.addCanary', async () => {
      const key = await vscode.window.showInputBox({
        prompt: 'Variable key for canary token',
        placeHolder: 'API_KEY',
        validateInput: v => v.trim() === '' ? 'Key cannot be empty' : null,
      });
      if (!key) return;
      try {
        await cliRun(['canary', 'add', key]);
        vscode.window.showInformationMessage(`Canary token added: ${key}`);
        securityProvider.refresh();
      } catch (e: any) {
        vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
      }
    }),
    vscode.commands.registerCommand('envforge.removeCanary', async (arg: any) => {
      const key = arg?.key ?? arg?.label;
      if (!key || typeof key !== 'string') return;
      const confirm = await vscode.window.showWarningMessage(
        `Remove canary token for "${key}"?`,
        { modal: true },
        'Remove',
      );
      if (confirm !== 'Remove') return;
      try {
        await cliRun(['canary', 'delete', key]);
        vscode.window.showInformationMessage(`Canary token removed: ${key}`);
        securityProvider.refresh();
      } catch (e: any) {
        vscode.window.showErrorMessage(`EnvForge: ${e.message}`);
      }
    }),
    vscode.commands.registerCommand('envforge.runLifecycleCheck', async () => {
      const { stdout } = await cliRun(['lifecycle', 'check']);
      showOutput('Lifecycle Check', stdout);
    }),
    vscode.commands.registerCommand('envforge.manageLifecycleRules', async () => {
      const { stdout } = await cliRun(['lifecycle', 'rule', 'list']);
      showOutput('Lifecycle Rules', stdout);
    }),
    vscode.commands.registerCommand('envforge.viewAuditTrail', async () => {
      const { stdout } = await cliRun(['audit', '-n', '100']);
      showOutput('Audit Trail', stdout);
    }),
    vscode.commands.registerCommand('envforge.showUnusedSecrets', async () => {
      const { stdout } = await cliRun(['analytics', 'unused']);
      showOutput('Unused Secrets', stdout);
    }),
    vscode.commands.registerCommand('envforge.showUsageSummary', async () => {
      const { stdout } = await cliRun(['analytics', 'summary']);
      showOutput('Usage Summary', stdout);
    }),
    vscode.commands.registerCommand('envforge.monitorStream', async () => {
      const bin = getEnvforgePath();
      const terminal = vscode.window.createTerminal('EnvForge Monitor');
      terminal.show();
      terminal.sendText(`${bin} monitor stream`);
    }),
  );
}

function showOutput(title: string, content: string) {
  const channel = vscode.window.createOutputChannel(`EnvForge: ${title}`);
  channel.append(content);
  channel.show();
}

function cliRun(args: string[]): Promise<{ stdout: string; stderr: string }> {
  const binary = getEnvforgePath();
  const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || process.cwd();
  return new Promise((resolve, reject) => {
    cp.execFile(binary, args, { cwd, timeout: 30000 }, (err, stdout, stderr) => {
      if (err && !stdout) {
        reject(new Error(stderr?.trim() || err.message));
      } else {
        resolve({ stdout: stdout || '', stderr: stderr || '' });
      }
    });
  });
}
