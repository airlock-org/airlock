// Harmless Shell actors and signals for the real extension under test.
class Signals {
 constructor(){this.handlers=new Map();this.next=1;}
 connect(signal,callback){const id=this.next++;this.handlers.set(id,{signal,callback});return id;}
 disconnect(id){this.handlers.delete(id);}
 emit(signal,...args){for(const {signal:name,callback} of [...this.handlers.values()])if(name===signal)callback(this,...args);}
}
class Actor extends Signals {
 constructor(properties={}){super();Object.assign(this,{reactive:false,can_focus:false},properties);this.children=[];this.classes=new Set();}
 add_child(child){assert.equal(child.parent??null,null);this.children.push(child);child.parent=this;}
 insert_child_above(child,sibling){this.children.splice(this.children.indexOf(sibling)+1,0,child);child.parent=this;}
 remove_child(child){this.children=this.children.filter(c=>c!==child);child.parent=null;}
 get_first_child(){return this.children[0]??null;}
 set_child(child){this.add_child(child);}
 destroy_all_children(){for(const child of [...this.children]){this.remove_child(child);child.destroy();}}
 destroy(){this.emit("destroy");this.destroyed=true;}
 add_constraint(constraint){this.constraint=constraint;}
 hide(){this.visible=false;}
 show(){this.visible=true;}
 get_transformed_position(){return [400,0];}
 get_transformed_size(){return [30,24];}
 get_preferred_width(){return [80,180];}
 get_preferred_height(){return [20,30];}
 set_position(x,y){this.position=[x,y];}
 get_style(){return this.style??null;}
 set_label(label){this.label=label;}
 add_style_class_name(name){this.classes.add(name);}
 remove_style_class_name(name){this.classes.delete(name);}
}
const allWindows=new Set();
class Workspace {
 constructor(manager){this.manager=manager;}
 index(){return this.manager.workspaces.indexOf(this);}
 list_windows(){return [...allWindows].filter(window=>window.workspace===this);}
 activate(){this.manager.current=this;this.manager.emit('workspace-switched');}
}
class Manager extends Signals {
 constructor(){super();this.workspaces=[new Workspace(this),new Workspace(this)];this.current=this.workspaces[0];}
 get n_workspaces(){return this.workspaces.length;}
 get_workspace_by_index(index){return this.workspaces[index];}
 get_active_workspace(){return this.current;}
 append_new_workspace(){const w=new Workspace(this);this.workspaces.push(w);this.emit('workspace-added');return w;}
 remove_workspace(workspace){this.workspaces.splice(workspace.index(),1);this.emit('workspace-removed');}
 reorder_workspace(workspace,index){this.workspaces.splice(workspace.index(),1);this.workspaces.splice(index,0,workspace);}
}
class Window extends Signals {
 constructor(workspace,kind=0){super();this.workspace=workspace;this.kind=kind;this.moves=0;allWindows.add(this);}
 get_window_type(){return this.kind;}
 get_client_type(){return 1;}
 is_on_all_workspaces(){return false;}
 get_transient_for(){return this.parent??null;}
 get_wm_class(){return this.wmClass??null;}
 get_wm_class_instance(){return this.wmInstance??null;}
 get_workspace(){return this.workspace;}
 located_on_workspace(workspace){return this.workspace===workspace;}
 change_workspace(workspace){this.moves++;this.workspace=workspace;this.emit('workspace-changed');}
 change_workspace_by_index(index){this.change_workspace(wm.get_workspace_by_index(index));}
 stick(){this.sticky=true;this.emit('notify::on-all-workspaces');}
 unstick(){this.sticky=false;this.emit('notify::on-all-workspaces');}
}
const files=new Map(), sources=new Map(), settings=new Map([
 ['org.gnome.mutter',{'dynamic-workspaces':true}],
 ['org.gnome.desktop.wm.preferences',{'num-workspaces':4}]]);
const reads=[];
class Cancellable {cancel(){this.cancelled=true;}is_cancelled(){return Boolean(this.cancelled);}}
class Settings {
 constructor({schema_id}){this.values=settings.get(schema_id);}
 get_boolean(key){return this.values[key];}set_boolean(key,value){this.values[key]=value;}
 get_int(key){return this.values[key];}set_int(key,value){this.values[key]=value;}
}
const inventoryFile={
 load_contents_async(cancellable,callback){reads.push({cancellable,callback});},
 load_contents_finish(result){if(result.error)throw Error('missing');return [true,new TextEncoder().encode(result.text)];}
};
const monitor=new Signals();monitor.cancel=()=>{monitor.cancelled=true;};
const wm=new Manager(), display=new Signals(), actors=[];
globalThis.global={workspace_manager:wm,window_manager:new Signals(),display,get_current_time:()=>42,get_pointer:()=>[0,0,0],get_window_actors:()=>actors};
class WorkspacePreview extends Actor {
 constructor(workspace){super();this._init(workspace);}
 _init(workspace){this.metaWorkspace=workspace;this._background=new Actor();this.add_child(this._background);}
}
class ThumbnailPreview extends Actor {
 constructor(workspace){super();this._init(workspace);}
 _init(workspace){this.metaWorkspace=workspace;}
}
class InjectionManager {
 constructor(){this.restore=[];}
 overrideMethod(prototype,name,factory){const original=prototype[name];this.restore.push(()=>prototype[name]=original);prototype[name]=factory(original);}
 clear(){for(const restore of this.restore.reverse())restore();}
}
const overview=Object.assign(new Signals(),{visible:false,toggles:0,allowed:true,shouldToggleByCornerOrButton(){return this.allowed;},
 hide(){this.visible=false;},
 toggle(){this.visible=!this.visible;this.toggles++;},
 _overview:{controls:{_thumbnailsBox:{_thumbnails:[]},_workspacesDisplay:{_workspacesViews:[]}}}});
const nativeIndicators=new Actor(),activities=new Actor({reactive:true,can_focus:true,accessible_name:'Activities'});
activities.add_child(nativeIndicators);
activities._clickGesture=new Signals();activities._clickGesture.connect('recognize',()=>{
 if(overview.shouldToggleByCornerOrButton())overview.toggle();});
activities.vfunc_scroll_event=event=>event;
activities.handleDragOver=source=>source;
const activations=[];
globalThis.shellMocks={
 Clutter:{ActorAlign:{CENTER:0},ModifierType:{SHIFT_MASK:1},BindConstraint:class {constructor(properties){Object.assign(this,properties);}},BindCoordinate:{SIZE:1,ALL:2}},Gio:{Settings,Cancellable,
 File:{new_for_path:path=>path.endsWith('inventory.json')?inventoryFile:{monitor_directory:()=>monitor}},
 FileMonitorFlags:{WATCH_MOVES:1},DBus:{session:{}},BusNameOwnerFlags:{NONE:0},
 DBusExportedObject:{wrapJSObject:()=>({export(){},unexport(){}})},
 bus_own_name_on_connection:()=>1,bus_unown_name(){}},
 GLib:{get_user_state_dir:()=>'/state',build_filenamev:parts=>parts.join('/'),mkdir_with_parents(){},
 file_get_contents:path=>{if(!files.has(path))throw Error('missing');return [true,new TextEncoder().encode(files.get(path))];},
 file_set_contents:(path,text)=>files.set(path,text),PRIORITY_DEFAULT:0,SOURCE_REMOVE:false,
 timeout_add:(_priority,_delay,callback)=>{const id=sources.size+1;sources.set(id,callback);return id;},
 Source:{remove:id=>sources.delete(id)},get_monotonic_time:()=>0,uuid_string_random:()=>String(sources.size)},
 Meta:{Window,WindowType:{NORMAL:0,DIALOG:1,MODAL_DIALOG:2},WindowClientType:{WAYLAND:1}},
 St:{BoxLayout:Actor,Widget:Actor,Label:Actor,Button:Actor,ThemeContext:{get_for_stage:()=>({scaleFactor:1})}},
 PanelMenu:{Button:Actor},DND:{DragMotionResult:{MOVE_DROP:2,CONTINUE:3,NO_DROP:0}},
 Main:{activateWindow(window,time){activations.push({window,time});window.workspace.activate();display.focus_window=window;overview.hide();},
 panel:{statusArea:{activities},addToStatusArea(name,indicator,index,side){this.placement={name,indicator,index,side};}},
 layoutManager:{addChrome(){},removeChrome(){},primaryMonitor:{x:0,y:0,width:1920,height:1080},findMonitorForActor(){return this.primaryMonitor;}},
 overview,sessionMode:new Signals(),wm:{_shouldAnimateActor:()=>true,actionMoveWindow(window,workspace){window.change_workspace(workspace);workspace.activate();}}},
 Workspace:{Workspace:WorkspacePreview,WorkspaceBackground:class {_updateBorderRadius(){}}},WorkspaceThumbnail:{WorkspaceThumbnail:ThumbnailPreview,ThumbnailsBox:class {_init(){} _updateIndicator(){} addThumbnails(){}}},InjectionManager,
 Extension:class {}
};
const inventoryText=JSON.stringify(Object.fromEntries([...inventory].map(([uuid,vm])=>[uuid,{uuid,...vm,label:vm.color}])));
const finishInventory=()=>reads.at(-1).callback(inventoryFile,{text:inventoryText});
