// Native focus state and lifetime races; Rust owns process and filesystem setup.
switch(process.env.AIRLOCK_JS_SCENARIO) {
case "focus_changes_suppression_and_late_results": {
const {FocusState, LOCAL, UNKNOWN, validateIdentity}=await import("./state.mjs");
const rendered = [];
const state = new FocusState(value => rendered.push(value));
const first = state.begin('files', false);
state.complete(first, {kind:'vm', name:'usr-home', color:'#3465a4'});
assert.equal(rendered.at(-1).name, 'usr-home');
const second = state.begin('browser', false);
assert.deepEqual(rendered.at(-1), UNKNOWN);
state.complete(first, {kind:'vm', name:'usr-home', color:'#3465a4'});
assert.deepEqual(rendered.at(-1), UNKNOWN);
state.complete(second, {kind:'vm', name:'usr-browser', color:'#cc0000'});
assert.equal(rendered.at(-1).name, 'usr-browser');
const menu = state.begin('browser', true);
assert.equal(rendered.at(-1), null);
state.complete(second, LOCAL);
assert.equal(rendered.at(-1), null);
const empty = state.begin(null, false);
assert.equal(rendered.at(-1), null);
state.complete(menu, LOCAL);
assert.equal(rendered.at(-1), null);
const local = state.begin('terminal', false);
state.complete(local, LOCAL);
assert.deepEqual(rendered.at(-1), LOCAL);
state.disable();
state.complete(local, UNKNOWN);
state.complete(empty, LOCAL);
assert.equal(rendered.at(-1), null);
assert.deepEqual(validateIdentity({kind:'vm',name:'<b>bad</b>',color:'#ffffff'}), UNKNOWN);
assert.deepEqual(validateIdentity({kind:'vm',name:'usr-home\n',color:'#ffffff'}), UNKNOWN);
assert.deepEqual(validateIdentity({kind:'vm',name:['usr-home'],color:'#ffffff'}), UNKNOWN);
assert.deepEqual(validateIdentity({kind:'absent'}), UNKNOWN);
assert.equal(validateIdentity({kind:'vm',name:'usr-home',color:'red; width:10000px'}).color, null);

break;
}
case "shell_update_cannot_publish_into_a_reenabled_instance": {
const {FocusState, WindowIdentityCache}=await import("./state.mjs");
globalThis.shellMocks = {
    Gio: {Cancellable: class {}},
    Meta: {WindowType: {DESKTOP: 1, DOCK: 2}, WindowClientType: {X11:0, WAYLAND:1}},
    Extension: class {},
};
const {default: Extension} = await import("./extension.mjs");
const extension = new Extension();
const renders = [];
const requests = [];
extension._state = new FocusState(value => renders.push(value));
extension._cache = new WindowIdentityCache();
extension._focused = {get_window_type: () => 0, get_client_type: () => 0};
extension._suppressed = () => false;
extension._cancelRead = () => {};
extension._resolve = () => new Promise(resolve => requests.push(resolve));
extension._update();
extension._state.disable();
extension._state = new FocusState(value => renders.push(value));
extension._update();
requests[1]({kind:'vm',name:'usr-browser',color:'#cc0000'});
await new Promise(resolve => setImmediate(resolve));
assert.equal(renders.at(-1).name, 'usr-browser');
requests[0]({kind:'vm',name:'usr-home',color:'#3465a4'});
await new Promise(resolve => setImmediate(resolve));
assert.equal(renders.at(-1).name, 'usr-browser');

break;
}
case "application_actors_and_settled_shell_focus": {
const {FocusState, WindowIdentityCache}=await import("./state.mjs");
let keyFocus = null;
const idle = [];
const removed = [];
const Main = {overview: {visible:false}, sessionMode: {isLocked:false,isGreeter:false}, modalCount:0};
globalThis.shellMocks = {
    Gio: {Cancellable: class {}},
    GLib: {PRIORITY_DEFAULT_IDLE:0, SOURCE_REMOVE:false,
        idle_add: (_priority, callback) => {idle.push(callback); return idle.length;},
        Source: {remove: id => removed.push(id)},
    },
    Meta: {WindowType: {DESKTOP:1,DOCK:2}, WindowClientType: {X11:0, WAYLAND:1}},
    Main,
    Extension: class {},
};
const calculator = {get_window_type:()=>0, get_client_type:()=>0, get_transient_for:()=>null,
    connect:()=>1, disconnect:()=>{}};
globalThis.global = {stage:{get_key_focus:()=>keyFocus}, display:{focus_window:calculator},
    window_group:{contains: actor=>actor.group==='window'},
    top_window_group:{contains: actor=>actor.group==='top-window'},
};
const {default: Extension} = await import("./extension.mjs");
const extension = new Extension();
for (const actor of [null, {group:'window'}, {group:'top-window'}]) {
    keyFocus=actor;
    assert.equal(extension._suppressed(), false);
}
keyFocus={group:'shell'};
assert.equal(extension._suppressed(), true);
keyFocus=null;
Main.modalCount=1;
assert.equal(extension._suppressed(), true);
Main.modalCount=0;
Main.overview.visible=true;
assert.equal(extension._suppressed(), true);
Main.overview.visible=false;
Main.sessionMode.isLocked=true;
assert.equal(extension._suppressed(), true);
Main.sessionMode.isLocked=false;
const rendered=[];
extension._state=new FocusState(value=>rendered.push(value));
extension._signals=[];
extension._windowSignals=[];
extension._cache=new WindowIdentityCache();
extension._cancelRead=()=>{};
let reads=0;
extension._resolve=async()=>{reads++; return {kind:'vm',name:'usr-home',color:'#3465a4'};};
// Grab/key-focus notifications arrive while the menu close handler is running.
Main.modalCount=1;
keyFocus={group:'shell'};
extension._queueUpdate();
extension._queueUpdate();
assert.equal(idle.length,1);
assert.equal(rendered.at(-1),null);
assert.equal(reads,0);
// Complete the close handler before the idle callback; refresh the actual app.
Main.modalCount=0;
keyFocus={group:'window'};
idle[0]();
await new Promise(resolve=>setImmediate(resolve));
assert.equal(reads,1);
assert.equal(rendered.at(-1).name,'usr-home');
// Disable removes an outstanding idle callback as well as pending reads/signals.
extension._indicator={destroy:()=>{}};
extension._queueUpdate();
extension.disable();
assert.deepEqual(removed,[2]);
assert.equal(rendered.at(-1),null);

break;
}
case "cached_switches_retry_unknowns_and_window_lifetime": {
const {FocusState, WindowIdentityCache, LOCAL, UNKNOWN}=await import("./state.mjs");
globalThis.shellMocks = {
    Gio: {Cancellable: class {
        cancelled = false;
        cancel() {this.cancelled = true;}
        is_cancelled() {return this.cancelled;}
    }},
    Meta: {WindowType: {DESKTOP:1,DOCK:2}, WindowClientType: {X11:0,WAYLAND:1}},
    Extension: class {},
};
const {default: Extension} = await import("./extension.mjs");
const extension = new Extension();
const rendered = [];
const reads = [];
let suppressed = false;
function window(xid, clientType = 0) {
    const signals = new Map();
    let id = 0;
    return {
        parent: null,
        get_client_type: () => clientType,
        get_window_type: () => 0,
        get_description: () => `${xid} untrusted title`,
        get_transient_for() {return this.parent;},
        connect(signal, callback) {signals.set(++id, callback); return id;},
        disconnect(id) {signals.delete(id);},
        close() {for (const callback of [...signals.values()]) callback();},
        signalCount: () => signals.size,
    };
}
const home = {kind:'vm',name:'usr-home',color:'#3465a4'};
const browser = {kind:'vm',name:'usr-browser',color:'#f57900'};
extension._cache = new WindowIdentityCache();
extension._render = value => rendered.push(value);
extension._state = new FocusState(value => rendered.push(value));
extension._suppressed = () => suppressed;
extension._read = (xid, cancellable) => new Promise(resolve => reads.push({xid,cancellable,resolve}));
const settle = () => new Promise(resolve => setImmediate(resolve));
function focus(window) {extension._focused = window; extension._update();}
const files = window('0x123');
const firefox = window('0x456');
focus(files);
assert.deepEqual(rendered.at(-1), UNKNOWN);
reads.at(-1).resolve(home);
await settle();
focus(firefox);
assert.deepEqual(rendered.at(-1), UNKNOWN); // Never use the previous window's cache.
reads.at(-1).resolve(browser);
await settle();
const warmStart = rendered.length;
for (let i = 0; i < 100; i++) {focus(files); focus(firefox);}
assert.equal(reads.length, 2); // 200 repeat switches spawn no reader.
assert.ok(rendered.slice(warmStart).every(value => value.kind === 'vm'));
assert.deepEqual(rendered.slice(warmStart, warmStart + 2), [home, browser]);
suppressed = true;
focus(firefox);
assert.equal(rendered.at(-1), null);
suppressed = false;
focus(firefox);
assert.deepEqual(rendered.at(-1), browser);
assert.equal(reads.length, 2);
focus(null);
assert.equal(rendered.at(-1), null);
const localStart = rendered.length;
focus(window('unused', 1));
assert.deepEqual(rendered.slice(localStart), [LOCAL]);
assert.equal(reads.length, 2);
// Closure invalidates an unfocused entry; reusing its XID does not reuse identity.
files.close();
assert.equal(files.signalCount(), 0);
const replacement = window('0x123');
focus(replacement);
assert.deepEqual(rendered.at(-1), UNKNOWN);
assert.equal(reads.length, 3);
reads.at(-1).resolve(UNKNOWN);
await settle();
focus(replacement);
assert.equal(reads.length, 4); // Unknown is retried, never cached.
reads.at(-1).resolve(home);
await settle();
// Changing a transient's parent must resolve the current parent, not a cached one.
const dialog = window('0x789');
dialog.parent = replacement;
focus(dialog);
reads.at(-1).resolve({kind:'absent'});
await settle();
assert.deepEqual(rendered.at(-1), home);
dialog.parent = firefox;
focus(dialog);
reads.at(-1).resolve({kind:'absent'});
await settle();
assert.deepEqual(rendered.at(-1), browser);
// Rapid focus changes cancel cold reads; late results cannot populate that cache.
const racing = window('0xabc');
focus(racing);
const cancelled = reads.at(-1);
focus(firefox);
cancelled.resolve(home);
await settle();
assert.deepEqual(rendered.at(-1), browser);
assert.equal(extension._cache.get(racing), null);
// A window destroyed during its read cannot be resurrected by the completion.
focus(racing);
const closing = reads.at(-1);
racing.close();
closing.resolve(home);
await settle();
assert.equal(extension._cache.get(racing), null);
// Disable/re-enable cannot leak a late result into either old or new caches.
const pending = window('0xdef');
focus(pending);
const late = reads.at(-1);
const oldCache = extension._cache;
oldCache.clear();
extension._state.disable();
extension._cache = new WindowIdentityCache();
extension._render = value => rendered.push(value);
extension._state = new FocusState(value => rendered.push(value));
focus(firefox);
assert.deepEqual(rendered.at(-1), UNKNOWN);
late.resolve(home);
await settle();
assert.equal(oldCache.get(pending), null);
assert.equal(extension._cache.get(pending), null);
assert.deepEqual(rendered.at(-1), UNKNOWN);
assert.equal(pending.signalCount(), 0);
assert.equal(replacement.signalCount(), 0);
reads.at(-1).resolve(browser);
await settle();
assert.deepEqual(rendered.at(-1), browser);
extension._cache.clear();
assert.equal(firefox.signalCount(), 0);

break;
}
default: throw new Error("unknown focus scenario");
}
