// Execute the original AttemptCapturePhase.start with local deterministic
// collaborators and stop before any delayed battle/capture callbacks run.
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { root } from "./local-files.mjs";

export function captureSource() {
	const require = createRequire(path.join(root, "original/package.json"));
	const ts = require("typescript");
	const file = "src/phases/attempt-capture-phase.ts";
	const source = fs.readFileSync(path.join(root, "original", file), "utf8");
	const ast = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true);
	const phase = ast.statements.find(
		(statement) => ts.isClassDeclaration(statement) && statement.name?.text === "AttemptCapturePhase",
	);
	const method = phase?.members.find(
		(member) => ts.isMethodDeclaration(member) && member.name.getText(ast) === "start",
	);
	if (!method) throw new Error("Pinned AttemptCapturePhase.start is missing");
	const code = `(()=>{
const PokemonPhase=class {start(){} end(){}};
const SubstituteTag={};
const StatusEffect={SLEEP:1};
const PokeballType={POKEBALL:0};
const globalScene=window.visualScene;
const audioManager={playSound(){}};
const timedEventManager={getShinyCatchMultiplier(){return 1;}};
const getPokeballCatchMultiplier=()=>1;
const getStatusEffectCatchRateMultiplier=()=>1;
const getCriticalCaptureChance=()=>0;
const getPokeballAtlasKey=()=>"pb";
const IS_TEST=true,isBeta=false,isDev=false;
function doPokeballBounceAnim(){}
class CaptureStartHarness extends PokemonPhase {
  getPokemon(){return globalScene.visualPokemon;}
  catch(){}
  failCatch(){}
  removePb(){}
  ${method.getText(ast)}
}
window.SourceCapture={throwPose(timeMs){
  const tweens=[];
  const sprite={x:0,y:0,setOrigin(x,y){this.originX=x;this.originY=y;return this;}};
  globalScene.pokeballCounts=[4];
  globalScene.addFieldSprite=(x,y)=>{sprite.x=x;sprite.y=y;return sprite;};
  globalScene.field={add(){},moveBelow(){}};
  globalScene.time={delayedCall(){}};
  globalScene.tweens={add(config){tweens.push(config);return {};}};
  globalScene.visualPokemon={hp:18,maxHp:18,species:{catchRate:45},status:null,y:84,
    getMaxHp(){return this.maxHp;},getTag(){return null;},isShiny(){return false;},
    randBattleSeedInt(){return 255;},getFieldPositionOffset(){return [0,0];}};
  const phase=new CaptureStartHarness();phase.pokeballType=PokeballType.POKEBALL;phase.start();
  const tween=tweens[0];
  const progress=Math.max(0,Math.min(1,timeMs/tween.duration));
  const ease=Phaser.Math.Easing.Cubic.Out(progress);
  return {x:sprite.x+(tween.x.value-sprite.x)*progress,
    y:sprite.y+(tween.y.value-sprite.y)*ease,
    originX:sprite.originX,originY:sprite.originY,duration:tween.duration,
    frame:"pb",ease:tween.y.ease,progress};
}};})();`;
	return ts.transpile(code, { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None });
}
