const scenario = process.env.AIRLOCK_JS_SCENARIO;
const snapshot = (phase = 'loading', attention = false) => ({
    schema: 'airlock.startup-panel.v1', boot_id: 'boot-1',
    complete: phase === 'ready', attention, detail: 'Loading applications',
    guests: [{name: 'usr-home', phase, detail: 'Application status'}],
});

if (scenario === 'snapshot_validation') {
    assert.equal(parseSnapshot(snapshot()).complete, false);
    assert.equal(parseSnapshot(snapshot('ready')).complete, true);
    for (const malformed of [
        {...snapshot(), complete: true},
        {...snapshot(), attention: 'false'},
        {...snapshot(), guests: [snapshot().guests[0], snapshot().guests[0]]},
        {...snapshot(), guests: [{name: 'usr-home\nother-vm', phase: 'ready', detail: ''}]},
        {...snapshot(), guests: [{name: 'usr-home', phase: 'unknown', detail: ''}]},
        {...snapshot(), detail: 'x'.repeat(513)},
    ])
        assert.throws(() => parseSnapshot(malformed));
    assert.equal(elapsedText(317), '5m 17s elapsed');
    assert.equal(elapsedText(-1), '0s elapsed');
} else {
    const extension = new StartupProgressExtension();
    extension.enable();
    assert.equal(extension._spinner.playing, true);
    assert.equal(extension._label.text, 'Starting Airlock');
    assert.equal(reads.length, 1);
    if (scenario === 'wait_for_application_publication') {
        reply(0, snapshot('starting'));
        tick();
        reply(1, snapshot());
        assert.equal(extension._button.visible, true);
        assert.equal(extension._spinner.playing, true);
        assert.equal(extension._rows.items.length, 1);
        tick();
        reply(2, snapshot('ready'));
        assert.equal(extension._spinner.playing, false);
        assert.equal(extension._label.text, 'Ready');
        tick();
        assert.equal(extension._button.visible, false);
        assert.equal(timers.size, 0);
        assert.equal(reads.length, 3);
    } else if (scenario === 'lost_status_retains_guests') {
        reply(0, snapshot());
        const rows = extension._rows.items;
        tick();
        reply(1, null, true);
        assert.equal(extension._rows.items, rows);
        assert.equal(extension._button.visible, true);
        assert.equal(extension._spinner.playing, true);
        assert.equal(extension._label.text, 'Startup status unavailable');
        assert.equal(extension._warning.visible, true);
        assert.match(extension._summary.label.text, /unavailable.*retrying/u);
        tick();
        reply(2, {...snapshot(), complete: true});
        assert.equal(extension._button.visible, true, 'malformed completion cannot hide startup');
    } else if (scenario === 'failures_remain_visible') {
        reply(0, snapshot('failed', true));
        assert.equal(extension._button.visible, true);
        assert.equal(extension._warning.visible, true);
        assert.equal(extension._spinner.playing, false);
        tick();
        reply(1, snapshot('ready'));
        assert.equal(extension._label.text, 'Ready');
    } else if (scenario === 'disable_rejects_stale_callbacks') {
        const firstButton = extension._button;
        extension.disable();
        assert.equal(firstButton.destroyed, true);
        assert.equal(reads[0].cancellable.cancelled, true);
        assert.equal(timers.size, 0);
        extension.enable();
        reply(0, snapshot('ready'));
        assert.equal(extension._button.visible, true);
        assert.equal(timers.size, 0);
        reply(1, snapshot());
        assert.equal(timers.size, 1);
    } else if (scenario === 'already_complete_stops_polling') {
        reply(0, snapshot('ready'));
        assert.equal(extension._button.visible, false);
        assert.equal(timers.size, 0);
    } else {
        throw new Error(`Unknown startup scenario ${scenario}`);
    }
    extension.disable();
    assert.equal(timers.size, 0);
}
