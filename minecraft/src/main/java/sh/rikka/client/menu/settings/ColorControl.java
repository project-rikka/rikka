package sh.rikka.client.menu.settings;

import java.util.HexFormat;
import java.util.Optional;
import java.util.function.DoubleConsumer;
import net.minecraft.util.ARGB;
import net.minecraft.util.Mth;
import org.jspecify.annotations.Nullable;
import os.rikka.Align;
import os.rikka.AnchorEdge;
import os.rikka.ColorRole;
import os.rikka.Rikka;
import os.rikka.TextDocument;
import os.rikka.TextRole;
import os.rikka.TextTone;
import os.rikka.View;
import sh.rikka.client.menu.Labels;
import sh.rikka.client.setting.ColorSetting;
import sh.rikka.client.ui.style.Colors;

final class ColorControl implements AutoCloseable {
	private static final float SWATCH_WIDTH = 40.0F;
	private static final float SWATCH_HEIGHT = 26.0F;
	private static final float PREVIEW_SIZE = 40.0F;
	private static final float EDITOR_WIDTH = 280.0F;
	private static final float LABEL_WIDTH = 88.0F;
	private static final float MAX_HUE = 360.0F;
	private static final int OPAQUE = 0xFF;

	private @Nullable Editing editing;

	void view(View view, ColorSetting setting) {
		String key = "color:" + setting.translationKey();
		int argb = setting.get().argb();
		view.box(() -> {
			})
			.width(SWATCH_WIDTH)
			.height(SWATCH_HEIGHT)
			.cornerRadius(Rikka.CORNER_ITEM)
			.background(Colors.of(argb))
			.border(1.0F, view.paletteColor(ColorRole.OUTLINE))
			.key(key)
			.tooltip(hex(argb, setting.isOpaque()))
			.onPress(context -> toggle(setting));
		Editing open = this.editing;
		if (open != null && open.setting == setting) {
			view.popover(() -> editor(view, open))
				.anchor(key, AnchorEdge.BELOW_END, 0.0F, Rikka.SPACE_SNUG)
				.onDismiss(context -> close());
		}
	}

	private void toggle(ColorSetting setting) {
		Editing open = this.editing;
		close();
		if (open == null || open.setting != setting) {
			this.editing = new Editing(setting);
		}
	}

	private void editor(View view, Editing open) {
		ColorSetting setting = open.setting;
		int argb = setting.get().argb();
		view.column(() -> {
				view.row(() -> {
						view.box(() -> {
							})
							.width(PREVIEW_SIZE)
							.height(PREVIEW_SIZE)
							.cornerRadius(Rikka.CORNER_ITEM)
							.background(Colors.of(argb))
							.border(1.0F, view.paletteColor(ColorRole.OUTLINE));
						view.textField(open.hex)
							.placeholder(setting.isOpaque() ? "#RRGGBB" : "#AARRGGBB")
							.invalid(open.invalid)
							.grow(1.0F)
							.onChange(context -> open.typed())
							.onSubmit(context -> open.submitted());
					})
					.gap(Rikka.SPACE_NORMAL)
					.alignItems(Align.CENTER)
					.fillWidth();
				channel(view, Labels.of("color.hue"), open.hue, MAX_HUE, value -> open.setHue((float) value));
				channel(view, Labels.of("color.saturation"), open.saturation, 1.0, value -> open.setSaturation((float) value));
				channel(view, Labels.of("color.brightness"), open.brightness, 1.0, value -> open.setBrightness((float) value));
				if (!setting.isOpaque()) {
					channel(view, Labels.of("color.opacity"), ARGB.alpha(argb) / (double) OPAQUE, 1.0, value -> open.setAlpha((float) value));
				}
				view.row(() -> {
						view.text(Labels.of("color.chroma")).grow(1.0F);
						view.toggle(setting.get().chroma()).onToggle((context, on) -> setting.setChroma(on));
					})
					.alignItems(Align.CENTER)
					.fillWidth();
			})
			.gap(Rikka.SPACE_NORMAL)
			.padding(Rikka.SPACE_ROOMY)
			.width(EDITOR_WIDTH);
	}

	private static void channel(View view, String label, double value, double max, DoubleConsumer set) {
		view.row(() -> {
				view.text(label).textStyle(TextRole.CAPTION).tone(TextTone.MUTED).width(LABEL_WIDTH);
				view.slider(value).range(0.0, max).grow(1.0F).onValue((context, changed) -> set.accept(changed));
			})
			.gap(Rikka.SPACE_SNUG)
			.alignItems(Align.CENTER)
			.fillWidth();
	}

	private static String hex(int argb, boolean opaque) {
		String digits = HexFormat.of().withUpperCase().toHexDigits(argb);
		return "#" + (opaque ? digits.substring(2) : digits);
	}

	private static Optional<Integer> parse(String text, boolean opaque) {
		String digits = text.strip();
		if (digits.startsWith("#")) {
			digits = digits.substring(1);
		}
		if (digits.length() != 6 && digits.length() != 8) {
			return Optional.empty();
		}
		if (!digits.chars().allMatch(HexFormat::isHexDigit)) {
			return Optional.empty();
		}
		int value = HexFormat.fromHexDigits(digits);
		boolean withAlpha = digits.length() == 8;
		return Optional.of(withAlpha && !opaque ? value : ARGB.opaque(value));
	}

	@Override
	public void close() {
		Editing open = this.editing;
		this.editing = null;
		if (open != null) {
			open.hex.close();
		}
	}

	private static final class Editing {
		private final ColorSetting setting;
		private final TextDocument hex;
		private float hue;
		private float saturation;
		private float brightness;
		private boolean invalid;

		private Editing(ColorSetting setting) {
			this.setting = setting;
			int argb = setting.get().argb();
			this.hex = new TextDocument(hex(argb, setting.isOpaque()));
			read(argb);
		}

		private void read(int argb) {
			float red = ARGB.redFloat(argb);
			float green = ARGB.greenFloat(argb);
			float blue = ARGB.blueFloat(argb);
			float max = Math.max(red, Math.max(green, blue));
			float delta = max - Math.min(red, Math.min(green, blue));
			this.brightness = max;
			this.saturation = max == 0.0F ? 0.0F : delta / max;
			if (delta > 0.0F) {
				float sector;
				if (max == red) {
					sector = Mth.positiveModulo((green - blue) / delta, 6.0F);
				} else if (max == green) {
					sector = (blue - red) / delta + 2.0F;
				} else {
					sector = (red - green) / delta + 4.0F;
				}
				this.hue = sector * (MAX_HUE / 6.0F);
			}
		}

		private void setHue(float hue) {
			this.hue = hue;
			write();
		}

		private void setSaturation(float saturation) {
			this.saturation = saturation;
			write();
		}

		private void setBrightness(float brightness) {
			this.brightness = brightness;
			write();
		}

		private void setAlpha(float alpha) {
			int argb = this.setting.get().argb();
			this.setting.set(ARGB.color(Math.round(alpha * OPAQUE), argb));
			this.hex.reset(hex(this.setting.get().argb(), this.setting.isOpaque()));
		}

		private void write() {
			int alpha = ARGB.alpha(this.setting.get().argb());
			this.setting.set(Mth.hsvToArgb(this.hue / MAX_HUE % 1.0F, this.saturation, this.brightness, alpha));
			this.hex.reset(hex(this.setting.get().argb(), this.setting.isOpaque()));
			this.invalid = false;
		}

		private void typed() {
			this.invalid = false;
			parse(this.hex.getText(), this.setting.isOpaque()).ifPresent(this::apply);
		}

		private void submitted() {
			Optional<Integer> parsed = parse(this.hex.getText(), this.setting.isOpaque());
			this.invalid = parsed.isEmpty();
			parsed.ifPresent(this::apply);
		}

		private void apply(int argb) {
			this.setting.set(argb);
			read(this.setting.get().argb());
		}
	}
}
