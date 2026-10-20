// Exercise real JavaScript models and Shell controllers through a Rust-owned process.
const scenario=JSON.parse(process.env.AIRLOCK_JS_ARGUMENTS ?? "{}");
switch(process.env.AIRLOCK_JS_SCENARIO) {
case "groups_follow_best_match_and_preserve_gnome_order": {
const inventory=parseInventory(JSON.stringify(snapshot()));
const ids=[app(AGENT), app(HOME,'firefox'), app(AGENT,'dev-terminal'), app(HOME)];
const groups=groupResults(ids,inventory);
assert.deepEqual(groups.map(g=>g.heading),['usr-assistant','usr-home']);
assert.deepEqual(groups[0].ids,[ids[0],ids[2]]);
assert.deepEqual(groups[1].ids,[ids[1],ids[3]]);
assert.equal(groups[0].stopped,true);
assert.equal(groups[0].color,'#cc0000');
assert.equal(groups.flatMap(g=>g.ids)[0],ids[0]);
assert.deepEqual(groupResults([],inventory),[]);

break;
}
case "many_matches_do_not_hide_another_vm": {
const inventory=parseInventory(JSON.stringify(snapshot()));
const ids=Array.from({length:30},(_,i)=>app(HOME,`application-${i}`));
ids.push(app(AGENT));
const groups=groupResults(ids,inventory);
assert.equal(groups.length,2);
assert.equal(groups[0].ids.length,30);
assert.deepEqual(groups[1].ids,[app(AGENT)]);

break;
}
case "names_colours_and_renames_use_inventory_uuid": {
const data=snapshot();
data[AGENT].label=data[HOME].label;
let inventory=parseInventory(JSON.stringify(data));
assert.equal(groupResults([app(HOME),app(AGENT)],inventory).length,2);
assert.equal(describeResult(app(HOME),'usr-home: Console: Long title',inventory).title,'Console: Long title');
assert.equal(describeResult(app(HOME),'Another full title',inventory).title,'Another full title');
data[HOME].name='usr-renamed';
inventory=parseInventory(JSON.stringify(data));
assert.equal(describeResult(app(HOME),'usr-renamed: Console',inventory).heading,'usr-renamed');
assert.equal(describeResult(app(HOME),'usr-renamed: Console',inventory).title,'Console');
delete data[HOME];
assert.equal(describeResult(app(HOME),'usr-home: Console',parseInventory(JSON.stringify(data))).heading,'Other applications');

break;
}
case "guest_names_cannot_supply_vm_identity_or_css": {
const data=snapshot();
for(const label of ['0x3465a4','#3465A4']) {
 data[HOME].label=label;
 assert.equal(parseInventory(JSON.stringify(data)).get(HOME).color,'#3465a4');
}
data[HOME].label='red; width:10000px';
data[AGENT].uuid=HOME;
const inventory=parseInventory(JSON.stringify(data));
assert.equal(inventory.get(HOME).color,null);
assert.equal(inventory.has(AGENT),false);
const local=describeResult('fake.desktop','usr-assistant: Console',inventory);
assert.equal(local.heading,'sys-gui');
assert.equal(local.title,'usr-assistant: Console');
assert.equal(local.color,null);
assert.equal(describeResult('shutdown','Power Off',inventory).heading,'System');
for(const name of ['<b>bad</b>','bad\n','a'.repeat(32)]) {
 data[HOME].name=name;
 assert.equal(parseInventory(JSON.stringify(data)).has(HOME),false);
}
for(const text of ['{','null','[]','1']) assert.throws(()=>parseInventory(text));

break;
}
case "rows_reuse_original_controls_and_wrap_full_names": {
const inventory=parseInventory(JSON.stringify(snapshot()));
const id=app(AGENT), action='shutdown';
const names={[id]:'usr-assistant: A very long application name', [action]:'Power Off'};
const display=new VMSearchResults(provider(names),{},()=>inventory);
await display.updateSearch([id,action],['app']);
const row=display.getFirstResult();
assert.equal(row._airlockTitle.text,'A very long application name');
assert.equal(row.accessible_name,names[id]);
assert.equal(row._airlockTitle.clutter_text.line_wrap,true);
assert.equal(row._airlockTitle.clutter_text.ellipsize,Pango.EllipsizeMode.NONE);
assert.equal(row.activate(),id);assert.equal(row.popup_menu(),id);
assert.equal(row.getDragActorSource(),row.icon);
assert.equal(row.icon.iconSize,32);
assert.equal(display._groups[0].get_child().get_children().at(-1).text,'Stopped');
assert.equal(display._resultDisplays[action].activate(),action);
assert.equal(display._groups[0].get_child().can_focus,false);
names[id]='usr-assistant: Renamed application';
await display.updateSearch([id],['app']);
assert.equal(display.getFirstResult(),row);
assert.equal(row.destroyed,undefined);
assert.equal(row._airlockTitle.text,'Renamed application');
await display.updateSearch([],[]);
assert.equal(display.visible,false);assert.equal(display.getFirstResult(),null);
display.clear();assert.equal(row.destroyed,true);

break;
}
case "late_queries_and_destroyed_displays_cannot_publish": {
const inventory=parseInventory(JSON.stringify(snapshot()));
const home=app(HOME), agent=app(AGENT);
const display=new VMSearchResults(provider({[home]:'usr-home: Console',[agent]:'usr-assistant: Console'}),{},()=>inventory);
const requests=[];
const original=display._ensureResultActors.bind(display);
display._ensureResultActors=ids=>new Promise(resolve=>requests.push(async()=>{await original(ids);resolve();}));
const old=display.updateSearch([home],['old']);
const latest=display.updateSearch([agent],['latest']);
await requests[1]();await latest;
const row=display.getFirstResult();
await requests[0]();await old;
assert.equal(display.getFirstResult(),row);assert.equal(row.activate(),agent);
const dying=display.updateSearch([home],['next']);
display._onDestroy();await requests[2]();await dying;
assert.equal(display.getFirstResult(),row);

break;
}
case "inventory_refresh_preserves_snapshot_and_ignores_stale_reads": {
const extension=new VMSearchExtension();
extension._active=true;extension._inventoryRead=0;extension._inventory=new Map();
const cancellable={is_cancelled:()=>false};extension._cancellable=cancellable;
const requests=[];let refreshed=0;
extension._refreshViews=()=>refreshed++;
extension._file={load_contents_async(cancellable,callback){requests.push(callback);}};
const result=text=>({load_contents_finish:()=>[true,new TextEncoder().encode(text)]});
const data=snapshot();
extension._loadInventory();extension._loadInventory();
requests[1](result(JSON.stringify(data)),{});
data[HOME].name='old-name';requests[0](result(JSON.stringify(data)),{});
assert.equal(extension._inventory.get(HOME).name,'usr-home');assert.equal(refreshed,1);
globalThis.logError=()=>{};
extension._loadInventory();requests[2](result('{'),{});
assert.equal(extension._inventory.get(HOME).name,'usr-home');
extension._loadInventory();
requests[3]({load_contents_finish(){throw {matches(domain,code){assert.equal(domain,42);return code===Gio.IOErrorEnum.NOT_FOUND;}};}},{});
assert.equal(extension._inventory.get(HOME).name,'usr-home');
extension._loadInventory();extension._active=false;
requests[4](result(JSON.stringify(data)),{});
assert.equal(extension._inventory.get(HOME).name,'usr-home');

break;
}
case "enable_disable_restores_search_and_leaves_other_providers": {
const names={[app(HOME)]:'usr-home: Console'};
const applications={...provider(names),id:'applications',display:new Actor()};
applications.display.clear=()=>{};
const files={id:'files',display:new Actor()};
const content=new Actor();content.add_child(applications.display);content.add_child(files.display);
const filesDisplay=files.display;
const view=new Search.SearchResultsView();
Object.assign(view,{_content:content,_providers:[applications,files],terms:[],_defaultResult:null,
 _setSelected(){},_focusChildChanged(){}});
const original=function(item) {if(!item.display){item.display=new Actor();item.display.clear=()=>{};this._content.add_child(item.display);}};
Search.SearchResultsView.prototype._ensureProviderDisplay=original;
// Replace the test's captured constructor implementations before enable.
Object.assign(shellMocks.Main,{overview:{_overview:{controls:{_searchController:{_searchResults:view}}}}});
shellMocks.Shell.AppSystem={get_default:()=>({connect:()=>1,disconnect(){}})};
Object.assign(shellMocks.GLib,{build_filenamev:parts=>parts.join('/'),get_user_state_dir:()=>'/state',mkdir_with_parents(){}});
Gio.Cancellable=class {cancel(){};is_cancelled(){return false;}};
Gio.FileMonitorFlags={WATCH_MOVES:1};
Gio.File={new_for_path:()=>({monitor_directory:()=>({connect(){},cancel(){}})})};
shellMocks.InjectionManager.prototype.overrideMethod=function(prototype,name,override) {
 this.restore=()=>prototype[name]=original;prototype[name]=override(original);
};
shellMocks.InjectionManager.prototype.clear=function(){this.restore();};
const extension=new VMSearchExtension();extension._loadInventory=()=>{};
extension.enable();
assert.ok(applications.display instanceof VMSearchResults);
assert.equal(files.display,filesDisplay);
assert.equal(content.children[0],applications.display);
const grouped=applications.display;
extension.disable();
assert.equal(Search.SearchResultsView.prototype._ensureProviderDisplay,original);
assert.equal(grouped.destroyed,true);assert.equal(grouped._alive,false);
assert.equal(applications.display instanceof VMSearchResults,false);
assert.equal(files.display,filesDisplay);
assert.equal(extension._views.size,0);

break;
}
default: throw new Error("unknown JavaScript scenario");
}
