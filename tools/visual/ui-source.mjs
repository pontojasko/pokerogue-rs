// Compile the actual pinned UI classes. Hidden tooltip/stat/identity collaborators
// are explicitly stubbed by the harness; no replacement layout oracle is used.
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { root } from "./local-files.mjs";
export function uiSource() {
	const require = createRequire(path.join(root, "original/package.json"));
	const ts = require("typescript");
	const files = [
		"src/enums/text-style.ts",
		"src/ui/text.ts",
		"src/ui/ui-theme.ts",
		"src/ui/battle-info/battle-info.ts",
		"src/ui/battle-info/player-battle-info.ts",
		"src/ui/battle-info/enemy-battle-info.ts",
		"src/ui/handlers/fight-ui-handler.ts",
	];
	const code = files
		.map((file) => {
			const text = fs.readFileSync(path.join(root, "original", file), "utf8");
			const ast = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
			return ast.statements
				.filter((s) => !ts.isImportDeclaration(s) && !ts.isExportDeclaration(s))
				.map((s) => s.getText(ast).replace(/^export\s+/, ""))
				.join("\n");
		})
		.join("\n");
	const shim = `const globalScene=window.visualScene; const Phaser=window.Phaser;
 class UiHandler {}
 const settings={isLegacyTheme:false,display:{uiWindowStyle:1}};
 const i18next={resolvedLanguage:'en',t:key=>key};
 const getLocalizedSpriteKey=key=>key; const PokemonType={UNKNOWN:-1};const StatusEffect={NONE:0};
 const Gender={GENDERLESS:0};const Stat={HP:0,ATK:1,DEF:2,SPATK:3,SPDEF:4,ACC:5,EVA:6,SPD:7};
 class BattleFlyout extends Phaser.GameObjects.Container {constructor(){super(globalScene,0,0);this.setVisible(false);}}
 `;
	const setup = `
 // These original constructor collaborators only build hidden interactive overlays.
 BattleInfo.prototype.constructIcons=function(){};
 BattleInfo.prototype.constructStatContainer=function(){
   this.statsContainer=globalScene.add.container(0,0).setVisible(false);this.add(this.statsContainer);
   this.statValuesContainer=globalScene.add.container(0,0);this.statsContainer.add(this.statValuesContainer);
 };
 for(const cls of [PlayerBattleInfo,EnemyBattleInfo]) cls.prototype.constructTypeIcons=function(){this.type1Icon=globalScene.add.container(0,0).setVisible(false);this.add(this.type1Icon);};
 window.SourceUI={PlayerBattleInfo,EnemyBattleInfo,FightUiHandler,addTextObject,TextStyle,addWindow};
 FightUiHandler.prototype.getMoveColor=()=>undefined;
 `;
	return (
		"(()=>{" +
		shim +
		ts.transpile(code, {
			target: ts.ScriptTarget.ES2022,
			module: ts.ModuleKind.None,
		}) +
		setup +
		"})();"
	);
}
