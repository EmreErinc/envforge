import * as vscode from 'vscode';
import { getClient } from './extension';

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

type GlyphKind = 'dot' | 'shield';

interface DecorationSet {
    red: vscode.TextEditorDecorationType;
    amber: vscode.TextEditorDecorationType;
    green: vscode.TextEditorDecorationType;
}

/**
 * Renders the AI-exposure heatmap in `.env*` files. Each env-var line
 * gets one gutter glyph + one overview-ruler tick:
 *
 *  - **Dot** (`Ø`-ish filled circle): plain exposure marker.
 *  - **Shield**: same color, shield silhouette — drawn instead of the
 *    dot when a canary tripwire is registered for the key. Makes the
 *    "this line is a tripwire" status visible at a glance without
 *    sacrificing the red/amber/green threat tier color.
 *
 * Data source: LSP custom request `envforge/exposureMap`.
 */
export class ExposureRenderer implements vscode.Disposable {
    private dots: DecorationSet;
    private shields: DecorationSet;
    private debounce: NodeJS.Timeout | undefined;

    constructor() {
        this.dots = makeDecorationSet('dot');
        this.shields = makeDecorationSet('shield');
    }

    /**
     * Trigger a refresh for the editor's document. Debounced 150 ms so
     * rapid keystrokes coalesce into a single LSP roundtrip.
     */
    scheduleRefresh(editor: vscode.TextEditor | undefined) {
        if (!editor) return;
        if (!isEnvFile(editor.document)) {
            this.clear(editor);
            return;
        }
        if (this.debounce) clearTimeout(this.debounce);
        this.debounce = setTimeout(() => this.refresh(editor), 150);
    }

    private async refresh(editor: vscode.TextEditor) {
        const client = getClient();
        if (!client) return;

        let response: ExposureMapResponse;
        try {
            response = await client.sendRequest<ExposureMapResponse>(
                'envforge/exposureMap',
                { uri: editor.document.uri.toString() }
            );
        } catch {
            this.clear(editor);
            return;
        }

        const bucket: Record<GlyphKind, Record<'red' | 'amber' | 'green', vscode.DecorationOptions[]>> = {
            dot: { red: [], amber: [], green: [] },
            shield: { red: [], amber: [], green: [] },
        };

        for (const entry of response.entries) {
            const lineLen = editor.document.lineAt(entry.line).text.length;
            const range = new vscode.Range(entry.line, 0, entry.line, lineLen);
            const banner = entry.canary
                ? `**EnvForge: ${entry.level.toUpperCase()} · CANARY ACTIVE**`
                : `**EnvForge AI Exposure: ${entry.level.toUpperCase()}**`;
            const hoverMessage = new vscode.MarkdownString(`${banner}\n\n${entry.reason}`);
            const opt: vscode.DecorationOptions = { range, hoverMessage };
            const glyph: GlyphKind = entry.canary ? 'shield' : 'dot';
            bucket[glyph][entry.level].push(opt);
        }

        editor.setDecorations(this.dots.red, bucket.dot.red);
        editor.setDecorations(this.dots.amber, bucket.dot.amber);
        editor.setDecorations(this.dots.green, bucket.dot.green);
        editor.setDecorations(this.shields.red, bucket.shield.red);
        editor.setDecorations(this.shields.amber, bucket.shield.amber);
        editor.setDecorations(this.shields.green, bucket.shield.green);
    }

    private clear(editor: vscode.TextEditor) {
        editor.setDecorations(this.dots.red, []);
        editor.setDecorations(this.dots.amber, []);
        editor.setDecorations(this.dots.green, []);
        editor.setDecorations(this.shields.red, []);
        editor.setDecorations(this.shields.amber, []);
        editor.setDecorations(this.shields.green, []);
    }

    dispose() {
        if (this.debounce) clearTimeout(this.debounce);
        for (const set of [this.dots, this.shields]) {
            set.red.dispose();
            set.amber.dispose();
            set.green.dispose();
        }
    }
}

function isEnvFile(doc: vscode.TextDocument): boolean {
    const fname = doc.uri.path.split('/').pop() ?? '';
    return (
        fname === '.env' ||
        fname.startsWith('.env.') ||
        fname.endsWith('.env') ||
        fname === 'env'
    );
}

function makeDecorationSet(glyph: GlyphKind): DecorationSet {
    return {
        red: makeDecoration('#d32f2f', glyph),
        amber: makeDecoration('#f9a825', glyph),
        green: makeDecoration('#2e7d32', glyph),
    };
}

function makeDecoration(
    color: string,
    glyph: GlyphKind,
): vscode.TextEditorDecorationType {
    const svg =
        glyph === 'shield'
            ? `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">
                 <path d="M8 1.5 L13 3.5 L13 8 Q13 11.5 8 14 Q3 11.5 3 8 L3 3.5 Z"
                       fill="${color}" stroke="${color}" stroke-width="1"
                       stroke-linejoin="round"/>
               </svg>`
            : `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">
                 <circle cx="8" cy="8" r="5" fill="${color}" stroke="${color}" stroke-width="1"/>
               </svg>`;
    const dataUri = vscode.Uri.parse(
        `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`
    );
    return vscode.window.createTextEditorDecorationType({
        gutterIconPath: dataUri,
        gutterIconSize: 'contain',
        overviewRulerColor: color,
        overviewRulerLane: vscode.OverviewRulerLane.Left,
        rangeBehavior: vscode.DecorationRangeBehavior.ClosedClosed,
        isWholeLine: false,
    });
}
