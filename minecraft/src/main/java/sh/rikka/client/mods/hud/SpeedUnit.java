package sh.rikka.client.mods.hud;

import java.util.Locale;

enum SpeedUnit {
	BLOCKS_PER_SECOND(1.0),
	KILOMETERS_PER_HOUR(3.6);

	private final double perBlockPerSecond;
	private final String translationSuffix;

	SpeedUnit(double perBlockPerSecond) {
		this.perBlockPerSecond = perBlockPerSecond;
		this.translationSuffix = "format." + name().toLowerCase(Locale.ROOT);
	}

	double fromBlocksPerSecond(double blocksPerSecond) {
		return blocksPerSecond * this.perBlockPerSecond;
	}

	String translationSuffix() {
		return this.translationSuffix;
	}
}
