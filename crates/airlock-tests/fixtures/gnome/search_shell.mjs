// Harmless Shell actors and signals for the real extension under test.
class Actor {
 constructor(...args) {this._init(...args);}
 _init(properties={}) {this.children=[]; this.visible=true; this.clutter_text={set(properties){Object.assign(this,properties);}}; Object.assign(this,properties);}
 add_child(child) {assert.equal(child.parent??null,null);this.children.push(child);child.parent=this;}
 remove_child(child) {this.children=this.children.filter(item=>item!==child);child.parent=null;}
 get_parent() {return this.parent??null;}
 get_child() {return this.children[0]??null;}
 set_child(child) {for(const old of [...this.children])this.remove_child(old);if(child)this.add_child(child);}
 get_children() {return this.children;}
 set_child_at_index(child,index) {this.remove_child(child);this.children.splice(index,0,child);child.parent=this;}
 set(properties) {Object.assign(this,properties);}
 set_size(width,height) {this.width=width;this.height=height;}
 set_style(style) {this.style=style;}
 hide() {this.visible=false;}
 show() {this.visible=true;}
 connect(name,callback) {this.callback=callback;return 1;}
 destroy() {this._onDestroy?.();this.destroyed=true;for(const child of [...this.children])child.destroy();this.parent?.remove_child(this);}
}
class Grid extends Actor {
 _init(provider,view) {
  super._init();this.provider=provider;this._resultsView=view;this._resultDisplays={};
  this._cancellable={cancel(){}};this._resultDisplayBin={child:new Actor()};
 }
 async _ensureResultActors(ids) {
  for(const id of ids) if(!this._resultDisplays[id]) this._resultDisplays[id]=this._createResultDisplay({id,name:this.provider.names[id]});
 }
 clear() {for(const row of Object.values(this._resultDisplays))row.destroy();this._resultDisplays={};this._clearResultDisplay();}
 _onDestroy() {}
}
const Clutter={ActorAlign:{CENTER:1},Orientation:{HORIZONTAL:0,VERTICAL:1}};
const St={Widget:Actor,BoxLayout:Actor,Label:Actor};
const Pango={WrapMode:{WORD_CHAR:1},EllipsizeMode:{NONE:0}};
const callbacks=[];
const Gio={IOErrorEnum:{NOT_FOUND:1,CANCELLED:2},io_error_quark:()=>42};
const Search={GridSearchResults:Grid,SearchResultsView:class {}};
globalThis.shellMocks={Clutter,St,Pango,Gio,Search,GObject:{registerClass:cls=>cls},GLib:{},Shell:{},Main:{},Extension:class {},InjectionManager:class {}};
function provider(names) {
 return {names,createResultObject(meta) {
  const row=new Actor();row.app=meta.id.endsWith('.desktop')?{get_name:()=>names[meta.id]}:null;row.metaInfo=meta;
  row.icon=new Actor();row.icon.label=new Actor();row.icon.setIconSize=function(size){assert.equal(this._setSizeManually,true);this.iconSize=size;};
  row.set_child(new Actor());row.activate=()=>meta.id;row.popup_menu=()=>meta.id;row.getDragActorSource=()=>row.icon;
  return row;
 }};
}
