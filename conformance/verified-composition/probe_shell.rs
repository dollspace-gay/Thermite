pub struct PlatformProbeState {
    pub owner: u64,
    pub generation: u64,
}

pub open spec fn represents(model: ProbeState, platform: PlatformProbeState) -> bool {
    model.owner == platform.owner && model.generation == platform.generation
}

pub fn boot_observation() -> (result: u64)
    ensures result == 1,
{
    let payload = TVecU64 { length: 0 };
    let model_before = ProbeState { owner: 7, generation: 0, payload };
    let platform_before = PlatformProbeState { owner: 7, generation: 0 };
    assert(represents(model_before, platform_before));

    let stepped = probe_step(model_before, ProbeEvent::Tick(9));
    let platform_after = PlatformProbeState {
        owner: platform_before.owner,
        generation: platform_before.generation + 1,
    };
    assert(represents(stepped.0, platform_after));
    match stepped.1 {
        ProbeAction::Record(value) => assert(value == 9),
        ProbeAction::Noop => assert(false),
    }
    platform_after.generation
}
