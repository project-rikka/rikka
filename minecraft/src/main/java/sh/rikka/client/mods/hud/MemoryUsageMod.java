package sh.rikka.client.mods.hud;

import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.ScreenPosition;

public final class MemoryUsageMod extends SimpleHudMod {
	private static final long BYTES_PER_MEBIBYTE = 1024L * 1024L;
	private static final long PERCENT = 100;

	private final EnumSetting<MemoryFormat> format = choice("format", MemoryFormat.PERCENT);

	public MemoryUsageMod(ScreenPosition defaultPosition) {
		super("memory_usage", defaultPosition, "memory", 18);
	}

	@Override
	protected String label() {
		Runtime runtime = Runtime.getRuntime();
		long used = runtime.totalMemory() - runtime.freeMemory();
		long max = runtime.maxMemory();
		return switch (this.format.get()) {
			case PERCENT -> translate("format.percent", used * PERCENT / max);
			case AMOUNT -> translate("format.amount", used / BYTES_PER_MEBIBYTE, max / BYTES_PER_MEBIBYTE);
		};
	}
}
