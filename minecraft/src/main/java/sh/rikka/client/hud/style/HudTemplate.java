package sh.rikka.client.hud.style;

public enum HudTemplate {
	BLUR(HudPreset.builder().surface(HudSurface.BLUR).fill(0x40000000).build()),
	LIQUID(HudPreset.builder().surface(HudSurface.LIQUID).fill(0x1F000000).outline(0x2EFFFFFF, 1.0).build()),
	THEME(HudPreset.builder().surface(HudSurface.THEME).fill(HudFill.THEME, 0.75).shadow(HudShadow.SHADOW, 0.25).text(HudInk.THEME).build()),
	MINECRAFT(HudPreset.builder().fill(0x66000000).textShadow().radius(0.0).build()),
	NORMAL(HudPreset.builder().fill(HudFill.ACCENT, 0.86).shadow(HudShadow.SHADOW, 0.35).build()),
	GLOW(HudPreset.builder().fill(HudFill.ACCENT, 0.86).shadow(HudShadow.GLOW, 0.6).build()),
	OUTLINE(HudPreset.builder().fill(0x64000000).accentOutline().shadow(HudShadow.SHADOW, 0.35).build()),
	OUTLINE_GLOW(HudPreset.builder().fill(0x64000000).accentOutline().shadow(HudShadow.GLOW, 0.6).build()),
	VANILLA(HudPreset.builder().fill(0x64000000).shadow(HudShadow.SHADOW, 0.35).build()),
	VANILLA_GLOW(HudPreset.builder().fill(0x64000000).shadow(HudShadow.GLOW, 0.6).build()),
	SHADOW(HudPreset.builder().shadow(HudShadow.SHADOW, 0.45).build()),
	DARK(HudPreset.builder().fill(0xDC141414).shadow(HudShadow.SHADOW, 0.35).text(HudInk.ACCENT).build()),
	LIGHT(HudPreset.builder().fill(0xDCF0F0F0).shadow(HudShadow.SHADOW, 0.25).text(HudInk.ACCENT).build());

	public static final HudTemplate DEFAULT = BLUR;

	private final HudPreset preset;

	HudTemplate(HudPreset preset) {
		this.preset = preset;
	}

	public HudPreset preset() {
		return this.preset;
	}
}
