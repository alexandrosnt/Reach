import { strict as assert } from 'node:assert';
import { buildFolderTree, canMoveFolder, folderPath } from '../src/lib/sessions/folder-tree.ts';

let passed = 0;
function test(name, fn) {
	try {
		fn();
		passed++;
	} catch (error) {
		console.error(`✗ ${name}\n  ${error.message}`);
		process.exitCode = 1;
	}
}

const folder = (id, name, parent_id = null, vault_id = null) => ({ id, name, parent_id, vault_id });
const session = (id, folder_id = null) => ({
	id, name: id, host: 'host', port: 22, username: 'root', auth_method: { type: 'Password' }, folder_id, tags: [],
});

const folders = [
	folder('grandchild', 'Grandchild', 'child'),
	folder('root', 'Root'),
	folder('child', 'Child', 'root'),
	folder('empty', 'Empty', 'root'),
];

test('builds a stable tree when children arrive before parents', () => {
	const tree = buildFolderTree(folders, [session('deep', 'grandchild'), session('loose')], () => true);
	assert.deepEqual(tree.roots.map((node) => node.folder.id), ['root']);
	assert.deepEqual(tree.roots[0].children.map((node) => node.folder.id), ['grandchild', 'child', 'empty'].filter((id) => id !== 'grandchild'));
	const child = tree.roots[0].children.find((node) => node.folder.id === 'child');
	assert.equal(child.children[0].folder.id, 'grandchild');
	assert.equal(tree.roots[0].totalSessions, 1);
	assert.deepEqual(tree.ungrouped.map((item) => item.id), ['loose']);
});

test('prunes empty branches but keeps ancestors of matching sessions', () => {
	const tree = buildFolderTree(folders, [session('deep', 'grandchild')]);
	assert.equal(tree.roots.length, 1);
	assert.deepEqual(tree.roots[0].children.map((node) => node.folder.id), ['child']);
	assert.equal(tree.roots[0].children[0].children[0].folder.id, 'grandchild');
});

test('shows requested empty folders', () => {
	const tree = buildFolderTree(folders, [], (item) => item.id === 'empty');
	assert.equal(tree.roots[0].folder.id, 'root');
	assert.deepEqual(tree.roots[0].children.map((node) => node.folder.id), ['empty']);
});

test('promotes orphan and cyclic folders to roots without duplicates', () => {
	const invalid = [folder('orphan', 'Orphan', 'missing'), folder('a', 'A', 'b'), folder('b', 'B', 'a')];
	const tree = buildFolderTree(invalid, [session('one', 'orphan'), session('two', 'a'), session('three', 'b')]);
	assert.deepEqual(tree.roots.map((node) => node.folder.id), ['orphan', 'a', 'b']);
	assert.equal(tree.roots.reduce((sum, node) => sum + node.totalSessions, 0), 3);
});

test('renders unambiguous breadcrumbs', () => {
	assert.equal(folderPath('grandchild', folders), 'Root / Child / Grandchild');
});

test('rejects moving a folder into itself, a descendant, or another vault', () => {
	const shared = [...folders, folder('other', 'Other', null, 'vault')];
	assert.equal(canMoveFolder('root', 'root', shared), false);
	assert.equal(canMoveFolder('root', 'grandchild', shared), false);
	assert.equal(canMoveFolder('root', 'other', shared), false);
	assert.equal(canMoveFolder('grandchild', null, shared), true);
	assert.equal(canMoveFolder('grandchild', 'root', shared), true);
});

console.log(`${passed} passed${process.exitCode ? ', with failures' : ''}`);
