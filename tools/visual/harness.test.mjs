import test from "node:test";
import assert from "node:assert/strict";
import vm from "node:vm";
import { arenaSource, localFile } from "./local-files.mjs";

test("real original ArenaBase builds Town layers without RNG or props", () => {
	const sprites = [];
	class Sprite {
		constructor(x, y, key) {
			this.x = x;
			this.y = y;
			this.key = key;
			this.texture = { frameTotal: 1 };
			sprites.push(this);
		}
		setOrigin(x, y) {
			this.origin = [x, y];
			return this;
		}
		setVisible(v) {
			this.visible = v;
			return this;
		}
		setTexture(key) {
			this.key = key;
			return this;
		}
		stop() {}
	}
	class Container {
		constructor() {
			this.children = [];
		}
		add(child) {
			this.children.push(child);
		}
	}
	const scene = {
		addFieldSprite: (...args) => new Sprite(...args),
		executeWithSeedOffset: (fn) => fn(),
		currentBattle: { waveIndex: 1 },
	};
	const context = {
		window: { visualScene: scene, Phaser: { GameObjects: { Container } } },
	};
	vm.runInNewContext(arenaSource(), context);
	const player = new context.window.ArenaBase(true);
	player.setBiome(0, 0);
	const enemy = new context.window.ArenaBase(false);
	enemy.setBiome(0, 0);
	assert.equal(player.base.key, "town_a");
	assert.equal(enemy.base.key, "town_b");
	assert.deepEqual(player.base.origin, [0, 0]);
	assert.equal(enemy.props.length, 3);
	assert.ok(enemy.props.every((p) => !p.visible));
});
test("file routing forbids workspace secrets and traversal", () => {
	assert.throws(() => localFile("http://localhost/../../.aws/credentials"));
	assert.throws(() => localFile("http://localhost/pokerogue-core/src/lib.rs"));
	assert.equal(
		localFile("http://localhost/tools/visual/scenes/battle-town.json")
			.contentType,
		"application/json",
	);
	assert.ok(
		localFile("http://localhost/original/node_modules/phaser/dist/phaser.js")
			.body.length > 100000,
	);
});

// Compile the real UI source to catch dependency/parse regressions before browser capture.
test("original HUD bundle contains actual constructors and HP number methods", () => {
  const source = localFile("http://localhost/ui-source.js").body;
  assert.doesNotThrow(() => new vm.Script(source));
  assert.ok(source.includes("class PlayerBattleInfo extends BattleInfo"));
  assert.ok(source.includes("class EnemyBattleInfo extends BattleInfo"));
  assert.ok(source.includes("setHpNumbers(hp, maxHp)"));
  assert.ok(source.includes("class FightUiHandler extends UiHandler"));
  assert.ok(source.includes("displayMoves()"));
});

test("pinned BattleAnim graphic-frame projector runs deterministically", () => {
	const source = localFile("http://localhost/anim-source.js").body;
	const context = { window: {} };
	assert.doesNotThrow(() => new vm.Script(source));
	vm.runInNewContext(source, context);
	const row = [{ x: 121.5, y: -62.5, zoomX: 100, zoomY: 100, angle: 0, mirror: false, target: 2, focus: 3, graphicFrame: 0, opacity: 100, visible: true }];
	const projected = context.window.SourceAnim.project([row])[0];
	assert.equal(projected.frame, 0);
	assert.equal(projected.opacity, 100);
	assert.equal(projected.x, 229.3984375);
	assert.equal(projected.y, 53.5);
	const tackle = JSON.parse(localFile("http://localhost/original/assets/battle-anims/tackle.json").body);
	const attack = context.window.SourceAnim.project(tackle.frames);
	assert.equal(attack.length, 6);
	assert.equal(attack[2].frame, 0);
	assert.equal(attack[3], null);
});

test("original AttemptCapturePhase.start supplies the fixed throw tween", () => {
	const source = localFile("http://localhost/capture-source.js").body;
	const context = {
		window: { visualScene: {} },
		Phaser: { Math: { Easing: { Cubic: { Out: (t) => 1 - (1 - t) ** 3 } } } },
	};
	assert.doesNotThrow(() => new vm.Script(source));
	vm.runInNewContext(source, context);
	const pose = context.window.SourceCapture.throwPose(250);
	assert.equal(pose.x, 126);
	assert.equal(pose.y, 24);
	assert.equal(pose.originX, 0.5);
	assert.equal(pose.originY, 0.625);
	assert.equal(pose.duration, 500);
});
