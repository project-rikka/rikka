package sh.rikka.client.mods.motionblur;

record VelocityStrength(float strength, int samples) {
	private static final int BASE_SAMPLES = 100;

	static VelocityStrength of(float strength, float fps, float refreshHz, boolean scalesWithRefreshRate) {
		if (!scalesWithRefreshRate) {
			return new VelocityStrength(strength, BASE_SAMPLES);
		}
		float framesPerRefresh = Math.max(1.0F, fps / refreshHz);
		return new VelocityStrength(strength * framesPerRefresh, (int) (BASE_SAMPLES * framesPerRefresh));
	}

	static VelocityStrength betweenBlendedFrames(float strength, float fps, float refreshHz) {
		float filler = fps > 0.0F ? Math.min(1.0F, strength * fps / Math.min(fps, refreshHz)) : strength;
		return new VelocityStrength(filler, Math.max(BASE_SAMPLES, Math.round(BASE_SAMPLES * filler)));
	}
}
