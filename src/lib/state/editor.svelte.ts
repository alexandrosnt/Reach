export interface EditorFile {
	connectionId: string;
	path: string;
	filename: string;
	content: string;
	originalContent: string;
	language: string;
}

let editorFile = $state<EditorFile | undefined>();

export function getEditorFile(): EditorFile | undefined {
	return editorFile;
}

export function openEditor(
	connectionId: string,
	path: string,
	filename: string,
	content: string
): void {
	editorFile = {
		connectionId,
		path,
		filename,
		content,
		originalContent: content,
		language: detectLanguage(filename)
	};
}

export function closeEditor(): void {
	editorFile = undefined;
}

export function updateEditorContent(content: string): void {
	if (editorFile) {
		editorFile.content = content;
	}
}

export function updateEditorLanguage(language: string): void {
	if (editorFile) {
		editorFile.language = language;
	}
}

export function markSaved(): void {
	if (editorFile) {
		editorFile.originalContent = editorFile.content;
	}
}

export function isEditorDirty(): boolean {
	if (!editorFile) return false;
	return editorFile.content !== editorFile.originalContent;
}

function detectLanguage(filename: string): string {
	const ext = filename.includes('.') ? filename.slice(filename.lastIndexOf('.')).toLowerCase() : '';
	const map: Record<string, string> = {
		'.js': 'javascript',
		'.jsx': 'javascript',
		'.mjs': 'javascript',
		'.cjs': 'javascript',
		'.ts': 'typescript',
		'.tsx': 'typescript',
		'.py': 'python',
		'.rs': 'rust',
		'.html': 'html',
		'.htm': 'html',
		'.css': 'css',
		'.scss': 'scss',
		'.less': 'less',
		'.json': 'json',
		'.md': 'markdown',
		'.markdown': 'markdown',
		'.yaml': 'yaml',
		'.yml': 'yaml',
		'.sh': 'shell',
		'.bash': 'shell',
		'.zsh': 'shell',
		'.c': 'c',
		'.h': 'c',
		'.cpp': 'cpp',
		'.hpp': 'cpp',
		'.cc': 'cpp',
		'.cxx': 'cpp',
		'.java': 'java',
		'.php': 'php',
		'.sql': 'sql',
		'.xml': 'xml',
		'.svg': 'xml',
		'.go': 'go',
		'.rb': 'ruby',
		'.conf': 'shell',
		'.ini': 'shell',
		'.toml': 'toml',
		'.lua': 'lua',
		'.r': 'r',
		'.swift': 'swift',
		'.kt': 'kotlin',
		'.kts': 'kotlin',
		'.dart': 'dart',
		'.vue': 'vue',
		'.svelte': 'html',
		'.dockerfile': 'dockerfile',
		'.tf': 'hcl',
	};

	// Check for dotfiles like Dockerfile, Makefile
	const basename = filename.toLowerCase();
	if (basename === 'dockerfile') return 'dockerfile';
	if (basename === 'makefile') return 'shell';
	if (basename === '.env' || basename.startsWith('.env.')) return 'shell';

	return map[ext] || 'text';
}
