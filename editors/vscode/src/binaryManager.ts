import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import * as https from 'https';
import * as zlib from 'zlib';

export const DEFAULT_VERSION = 'v0.2.2';

export function getManagedBinaryDir(): string {
    const dir = path.join(os.homedir(), '.envforge', 'bin');
    if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true });
    }
    return dir;
}

export function getManagedBinaryPath(): string {
    const isWindows = process.platform === 'win32';
    const name = isWindows ? 'envforge.exe' : 'envforge';
    return path.join(getManagedBinaryDir(), name);
}

export function copyLocalBuildIfAvailable(): boolean {
    const isWindows = process.platform === 'win32';
    const binName = isWindows ? 'envforge.exe' : 'envforge';
    const managedPath = getManagedBinaryPath();

    if (vscode.workspace.workspaceFolders) {
        for (const folder of vscode.workspace.workspaceFolders) {
            const releasePath = path.join(folder.uri.fsPath, 'target', 'release', binName);
            const debugPath = path.join(folder.uri.fsPath, 'target', 'debug', binName);

            const source = fs.existsSync(releasePath) ? releasePath : fs.existsSync(debugPath) ? debugPath : undefined;
            if (source) {
                try {
                    fs.copyFileSync(source, managedPath);
                    fs.chmodSync(managedPath, 0o755);
                    return true;
                } catch {
                    // Ignore copy error and continue
                }
            }
        }
    }
    return false;
}

export function findBinaryPath(): string | undefined {
    const config = vscode.workspace.getConfiguration('envforge');
    const customPath = config.get<string>('path', '');
    if (customPath && fs.existsSync(customPath)) {
        return customPath;
    }

    const home = os.homedir();
    const isWindows = process.platform === 'win32';
    const binName = isWindows ? 'envforge.exe' : 'envforge';

    const workspaceCandidates: string[] = [];
    if (vscode.workspace.workspaceFolders) {
        for (const folder of vscode.workspace.workspaceFolders) {
            workspaceCandidates.push(path.join(folder.uri.fsPath, 'target', 'release', binName));
            workspaceCandidates.push(path.join(folder.uri.fsPath, 'target', 'debug', binName));
        }
    }

    const managedPath = getManagedBinaryPath();
    const candidates = [
        ...workspaceCandidates,
        managedPath,
        path.join(home, '.cargo', 'bin', binName),
        '/usr/local/bin/' + binName,
        '/opt/homebrew/bin/' + binName,
    ];

    for (const p of candidates) {
        if (fs.existsSync(p)) {
            try {
                fs.accessSync(p, fs.constants.X_OK);
                return p;
            } catch {
                // Not executable or accessible
            }
        }
    }

    return undefined;
}

export function getTargetTriple(): string | undefined {
    const platform = process.platform;
    const arch = process.arch;

    const archStr = arch === 'arm64' ? 'aarch64' : arch === 'x64' ? 'x86_64' : undefined;
    if (!archStr) return undefined;

    let osStr: string | undefined;
    if (platform === 'darwin') {
        osStr = 'apple-darwin';
    } else if (platform === 'linux') {
        osStr = 'unknown-linux-gnu';
    } else if (platform === 'win32') {
        osStr = 'pc-windows-msvc';
    }

    if (!osStr) return undefined;
    return `${archStr}-${osStr}`;
}

function downloadFile(url: string, destPath: string, progressCallback?: (downloaded: number, total: number) => void): Promise<void> {
    return new Promise((resolve, reject) => {
        const request = (currentUrl: string) => {
            https.get(currentUrl, (response) => {
                if (response.statusCode && response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
                    request(response.headers.location);
                    return;
                }

                if (response.statusCode !== 200) {
                    reject(new Error(`Failed to download binary: HTTP ${response.statusCode}`));
                    return;
                }

                const total = parseInt(response.headers['content-length'] || '0', 10);
                let downloaded = 0;
                const fileStream = fs.createWriteStream(destPath);

                response.on('data', (chunk) => {
                    downloaded += chunk.length;
                    if (progressCallback) {
                        progressCallback(downloaded, total);
                    }
                });

                response.pipe(fileStream);

                fileStream.on('finish', () => {
                    fileStream.close(() => resolve());
                });

                fileStream.on('error', (err) => {
                    fs.unlink(destPath, () => reject(err));
                });
            }).on('error', (err) => {
                reject(err);
            });
        };

        request(url);
    });
}

export async function downloadAndInstallBinary(progress?: vscode.Progress<{ message?: string; increment?: number }>): Promise<boolean> {
    const triple = getTargetTriple();
    if (!triple) {
        vscode.window.showErrorMessage(`Unsupported OS or architecture (${process.platform} ${process.arch}) for auto-download.`);
        return false;
    }

    const isWindows = process.platform === 'win32';
    const ext = isWindows ? 'zip' : 'tar.gz';
    const downloadUrl = `https://github.com/emreerinc/envforge/releases/download/${DEFAULT_VERSION}/envforge-${DEFAULT_VERSION}-${triple}.${ext}`;

    const managedDir = getManagedBinaryDir();
    const tempFile = path.join(managedDir, `envforge-download.${ext}`);
    const targetFile = getManagedBinaryPath();

    try {
        if (progress) {
            progress.report({ message: `Downloading EnvForge CLI (${DEFAULT_VERSION})...` });
        }

        await downloadFile(downloadUrl, tempFile);

        if (progress) {
            progress.report({ message: 'Extracting EnvForge CLI binary...' });
        }

        if (ext === 'tar.gz') {
            const gzData = fs.readFileSync(tempFile);
            const decompressed = zlib.gunzipSync(gzData);
            fs.writeFileSync(targetFile, decompressed);
        } else {
            fs.copyFileSync(tempFile, targetFile);
        }

        if (fs.existsSync(tempFile)) {
            fs.unlinkSync(tempFile);
        }

        fs.chmodSync(targetFile, 0o755);
        return true;
    } catch (err: any) {
        if (fs.existsSync(tempFile)) {
            fs.unlinkSync(tempFile);
        }
        console.error('Failed to download EnvForge binary:', err);
        return false;
    }
}

export async function ensureBinaryWithProgress(): Promise<boolean> {
    return await vscode.window.withProgress(
        {
            location: vscode.ProgressLocation.Notification,
            title: 'EnvForge: Downloading CLI Binary',
            cancellable: false,
        },
        async (progress) => {
            const success = await downloadAndInstallBinary(progress);
            if (success) {
                vscode.window.showInformationMessage('EnvForge CLI binary installed successfully!');
            } else {
                vscode.window.showErrorMessage('Could not download EnvForge CLI binary. Please check network connection or install CLI manually.');
            }
            return success;
        }
    );
}
