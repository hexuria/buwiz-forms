import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root=new URL("../",import.meta.url);
const schema=JSON.parse(fs.readFileSync(new URL("app/forms/forms-schema.json",root))).find(f=>f.c==="1601C");
const cat=schema.parts.flatMap((p,pi)=>p.items.map((it,ii)=>({it,pi,ii}))).find(x=>(x.it.f||[]).some(f=>/CatAgent_P$/.test(f.k)));
assert.ok(cat,"real 1601C category item");
const keys=cat.it.f.filter(f=>f.t==="radio").map(f=>f.k);
const traces=[], entities=[], tasks=[];
let rootDirty=true, owner=null, previewCalls=0, column=1056, errors=[], holdValidation=false, finishValidation;
const checkButtons=[], chips=[];
class View {}
function context(entity=null){
 return {notify(){if(entity)entity.dirty=true;else rootDirty=true;},
 new(Class,props){const e={dirty:true,released:false,set_props(p){e.props=p;e.view.update?.(p);e.dirty=true;},release(){e.released=true;return true;}};
 e.view=new Class(props);e.props=props;entities.push(e);const old=owner;owner=e;e.view.init?.(props,context(e));owner=old;return e;},
 spawn(fn){const p=Promise.resolve(fn(context(entity)));tasks.push(p);return {cancel(){}};},
 timer:{after(){return {cancel(){}};},every(){return {cancel(){}};}},
 theme(){return {appearance:"light",is_dark:false,colors:{}};}
 };
}
const clicks=new Map();
function element(){
 const creationOwner=owner;
 const target={};
 let el=new Proxy(target,{get(o,k){
 if(k==="then")return undefined;
 return (...args)=>{
 if(k==="id"){o.id=args[0];if(String(o.id).startsWith("fp-item-"))traces.push(o.id);}
 if(k==="on_click")clicks.set(o.id,{fn:args[0],entity:creationOwner});
 if(k==="when"&&args[0])args[1](el);
 if(k==="when_some"&&args[0]!=null)args[1](el,args[0]);
 if(k==="child")mount(args[0]);
 if(k==="children")for(const c of args[0])mount(c);
 return el;
 };}});
 return el;
}
function mount(e){if(e?.view && e.dirty&&!e.released){e.dirty=false;const old=owner;owner=e;e.view.render(context(e));owner=old;}}
const InputState={new({value=""}={}){let text=value;return {value:()=>text,set_value:v=>{text=v;},on(){},release(){return true;}};}};
const contextVM=vm.createContext({console,Map,Set,JSON,window:{viewport_size:()=>({width:1400,height:900})}});
function mock(name,exports){const m=new vm.SyntheticModule(Object.keys(exports),function(){for(const [k,v]of Object.entries(exports))this.setExport(k,v);},{context:contextVM,identifier:name});return m;}
const kit=mock("gpui-kit",{div:element,svg:element,View});
const base=mock("gpui-base",{InputState,CalendarState:{},Popup:{new:element}});
const components=mock("gpui-component",{Spinner:{new:element}});
const uiExports={tokens:()=>new Proxy({},{get:()=>"#000"}),SPACE:[0,4,8,12,16,20,24],RADIUS:{},SIZE:{validationLine:18},BREAK:{formNav:760}};
for(const n of ["Text","Line","Code","Icon","AppButton","IconButton","Avatar","Chip","Switch","Checkbox","TextField","Dropdown","PageHeader","MetaLine","Overlay","CheckboxLabel"])uiExports[n]=element;
uiExports.AppButton=(_cx,id,label,opts={})=>{
 if(id==="fp-check")checkButtons.push({label,ariaLabel:opts.ariaLabel,leading:opts.leading});
 return element();
};
uiExports.Chip=(_cx,label)=>{chips.push(label);return element();};
const uiModule=mock("./ui.js",uiExports);
const store=mock("./store.js",{formatTin:x=>x,errorText:e=>String(e&&e.message?e.message:e)});
const periods=mock("./periods.js",{periodLabel:()=>"March 2026"});
const mod=new vm.SourceTextModule(fs.readFileSync(new URL("app/shell/form_page.js",root),"utf8"),{context:contextVM});
await mod.link(s=>({"gpui-kit":kit,"gpui-base":base,"gpui-component":components,"./ui.js":uiModule,"./store.js":store,"./periods.js":periods}[s]));
await mod.evaluate();
let values=Object.fromEntries(keys.map((k,i)=>[k,i===0?"true":"false"]));
const app={prefs:{},branchFor:()=>null,taxpayers:()=>[],layout:()=>({column}),invalidate:()=>{rootDirty=true;},toast(){rootDirty=true;},store:{templates:()=>[],call(name,...args){
 if(name==="screen_form_schema")return schema;
 if(name==="screen_form_open")return Promise.resolve({draft:{id:1},status:"Draft",editable:true,values:{...values},locks:{},errors:[]});
 if(name==="screen_form_fill"){values={...values,...args[2]};return{draft:{id:1},values:{...values},options:{},rejected:{}};}
 if(name==="screen_form_validate"){
  const result={draft:args[1],values:{...values},options:{},errors};
  if(holdValidation)return new Promise(resolve=>{finishValidation=()=>resolve(result);});
  return Promise.resolve(result);
 }
 if(name==="screen_form_preview_sync"){previewCalls++;return true;}
 throw Error(name);
 }}};
app.formUi=new mod.namespace.FormPageActions(app);
const cx=context();
app.formUi.open({code:"1601C",tin:"12345678800000",year:2026,period:3},cx);
async function settle(){while(tasks.length)await Promise.all(tasks.splice(0));}
await settle();
app.formUi.jumpTo(cat.pi,cx);
app.formUi.state.previewOpen=true;
function frame(){if(rootDirty){rootDirty=false;owner=null;mod.namespace.renderFormPage(cx,app,column);}for(const e of entities)mount(e);}
frame();traces.length=0;
for(let i=0;i<6;i++){
 const key=keys[(i+1)%2],click=clicks.get("fp-"+key);assert.ok(click,"radio click handler");
 const old=owner;owner=click.entity;click.fn({},context(click.entity));owner=old;
 frame();await settle();frame();
 assert.equal(app.formUi.state.values[key],"true");
}
const rebuilt=[...new Set(traces)];
console.log(JSON.stringify({toggleCount:6,rowBuilds:traces.length,distinctRows:rebuilt.length,previewCalls,categoryRow:"fp-item-"+cat.pi+":"+cat.ii}));
assert.deepEqual(rebuilt,["fp-item-"+cat.pi+":"+cat.ii],"toggling category must not rebuild unrelated form rows");

// A repeated answer with unchanged validation must not rebuild any row.
traces.length=0;
app.formUi.choose({...values},cx);
frame();await settle();frame();
assert.equal(traces.length,0,"no-op answer");

// Other affected rows must refresh: computed/dependent values and messages.
const dependent=schema.parts[cat.pi].items[0].f[0].k;
values={...values,[dependent]:"456"};
errors=[{field:dependent,keys:[dependent],message:"Enter a valid TIN"}];
app.formUi.validate(cx);
frame();await settle();frame();
assert.deepEqual([...new Set(traces)],["fp-item-"+cat.pi+":0"],"dependent value/message refresh");
assert.equal(app.formUi.state.messages[cat.pi+":0"],"Enter a valid TIN");

// Stable field state is cached across ordinary parent notifications.
traces.length=0;rootDirty=true;frame();
assert.equal(traces.length,0,"parent redraw retains row snapshots");

// Resize invalidates the visible rows once; another layout poll does nothing.
column=900;app.formUi.refreshLayout();frame();
assert.equal(new Set(traces).size,schema.parts[cat.pi].items.length,"resize updates all visible widths");
traces.length=0;app.formUi.refreshLayout();frame();
assert.equal(traces.length,0,"idle layout poll");

// An edit while a part is folded must appear when that part reopens.
app.formUi.jumpTo(0,cx);frame();
values={...values,[dependent]:"789"};
app.formUi.validate(cx);await settle();frame();
traces.length=0;app.formUi.jumpTo(cat.pi,cx);frame();
assert.ok(traces.includes("fp-item-"+cat.pi+":0"),"reopened row refreshes");

const previous=[...app.formUi.state.rowViews.values()];
app.formUi.reload(cx);await settle();frame();
assert.ok(previous.every(r=>r.entity.released),"old return releases retained rows");
// Validation activity belongs to the retained Check button, never a banner.
holdValidation=true;traces.length=0;checkButtons.length=0;
app.formUi.validate(cx);frame();
assert.equal(checkButtons.at(-1).label,"Check","button label remains stable");
assert.match(checkButtons.at(-1).ariaLabel,/validation in progress/);
assert.ok(checkButtons.at(-1).leading,"spinner occupies the existing icon slot");
assert.equal(traces.length,0,"validation spinner does not rebuild field rows");
finishValidation();holdValidation=false;await settle();frame();
assert.equal(checkButtons.at(-1).ariaLabel,"Check","button returns to its idle icon");
assert.ok(!chips.some(s=>String(s).includes("Checking")),"no Checking chip anywhere");
console.log("PASS: unchanged rows, dependent values/errors, resize, folded sections and return reload.");
