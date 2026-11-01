// Compile the original animation coordinate projector in isolation. Battle
// phase execution is deliberately outside this bounded visual fixture.
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { root } from "./local-files.mjs";

export function animSource() {
	const require = createRequire(path.join(root, "original/package.json"));
	const ts = require("typescript");
	const file = "src/data/battle-anims.ts";
	const source = fs.readFileSync(path.join(root, "original", file), "utf8");
	const ast = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true);
	const helpers = ast.statements
		.filter((statement) =>
			ts.isFunctionDeclaration(statement) &&
			["transformPoint", "yAxisIntersect", "repositionY", "isReversed"].includes(statement.name?.text),
		)
		.map((statement) => statement.getText(ast))
		.join("\n");
	const battleAnim = ast.statements.find(
		(statement) => ts.isClassDeclaration(statement) && statement.name?.text === "BattleAnim",
	);
	const method = battleAnim?.members.find(
		(member) => ts.isMethodDeclaration(member) && member.name.getText(ast) === "getGraphicFrameData",
	);
	if (!method) throw new Error("Pinned BattleAnim.getGraphicFrameData is missing");
	const code = `
const AnimFrameTarget={USER:0,TARGET:1,GRAPHIC:2};
const AnimFocus={TARGET:1,USER:2,USER_TARGET:3};
${helpers}
class OriginalFrameProjector {
  isOppAnim(){return false;}
  ${method.getText(ast)}
}
window.SourceAnim={project(rows){
  const projector=new OriginalFrameProjector();
  projector.srcLine=[106,116,234,52];
  // BattleAnim.play stores sprite baselines here, then subtracts each sprite's
  // half-height inside getGraphicFrameData, exactly as the source does.
  projector.dstLine=[106,148,236,84];
  projector.user={x:106,y:148,getSprite:()=>({displayHeight:64})};
  projector.target={x:236,y:84,getSprite:()=>({displayHeight:64})};
  return rows.map(row=>{
		  const normalized=row.map(frame=>({...frame,angle:frame.angle??0,zoomX:frame.zoomX??100,zoomY:frame.zoomY??100}));
		  const item=normalized.find(frame=>frame.target===AnimFrameTarget.GRAPHIC);
		  if(!item)return null;
		  const positions=projector.getGraphicFrameData(normalized).get(AnimFrameTarget.GRAPHIC);
    const point=positions.get(0);
    return {x:point.x,y:point.y,scaleX:point.scaleX,scaleY:point.scaleY,angle:point.angle,frame:item.graphicFrame,opacity:item.opacity,visible:item.visible};
  });
}};`;
	return ts.transpile(code, { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None });
}
