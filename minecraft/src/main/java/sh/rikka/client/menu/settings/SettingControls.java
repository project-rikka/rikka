package sh.rikka.client.menu.settings;

import com.mojang.blaze3d.platform.InputConstants;
import java.math.BigDecimal;
import java.util.List;
import java.util.Locale;
import java.util.function.DoubleConsumer;
import os.rikka.Align;
import os.rikka.ButtonVariant;
import os.rikka.CheckState;
import os.rikka.Element;
import os.rikka.Rikka;
import os.rikka.TextAlignment;
import os.rikka.TextRole;
import os.rikka.TextTone;
import os.rikka.View;
import sh.rikka.client.menu.Labels;
import sh.rikka.client.setting.Anchor;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ColorSetting;
import sh.rikka.client.setting.DoubleRangeSetting;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.EnumSetSetting;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.IntRangeSetting;
import sh.rikka.client.setting.IntSetting;
import sh.rikka.client.setting.KeybindSetting;
import sh.rikka.client.setting.LanguageListSetting;
import sh.rikka.client.setting.LanguageSetting;
import sh.rikka.client.setting.PositionSetting;
import sh.rikka.client.setting.RegistrySetSetting;
import sh.rikka.client.setting.ScreenPosition;
import sh.rikka.client.setting.Setting;
import sh.rikka.client.setting.StringListSetting;
import sh.rikka.client.setting.StringSetting;

public final class SettingControls implements AutoCloseable {
	private static final int SEGMENTED_LIMIT = 3;
	private static final float SEGMENT_WIDTH = 88.0F;
	private static final int DOUBLE_DECIMALS = 2;
	private static final float SLIDER_WIDTH = 180.0F;
	private static final float VALUE_WIDTH = 56.0F;
	private static final float COMBO_WIDTH = 200.0F;
	private static final float KEY_WIDTH = 132.0F;
	private static final float COORDINATE_WIDTH = 96.0F;
	private static final float RANGE_LABEL_WIDTH = 72.0F;
	private static final double COORDINATE_STEP = 1.0;
	private static final int COORDINATE_DECIMALS = 1;

	private final KeyCapture capture = new KeyCapture();
	private final ColorControl colors = new ColorControl();
	private final TextControls texts = new TextControls();
	private final RegistryControl registries = new RegistryControl();
	private final LanguageControl languages = new LanguageControl();

	public KeyCapture capture() {
		return this.capture;
	}

	public static boolean isWide(Setting<?, ?> setting) {
		return setting instanceof StringListSetting
			|| setting instanceof EnumSetSetting<?>
			|| setting instanceof IntRangeSetting
			|| setting instanceof DoubleRangeSetting
			|| setting instanceof PositionSetting
			|| setting instanceof LanguageListSetting;
	}

	public void control(View view, Setting<?, ?> setting) {
		switch (setting) {
			case BoolSetting bool -> view.toggle(bool.get()).onToggle((context, on) -> bool.set(on));
			case IntSetting integer -> number(view, integer.get(), integer.min(), integer.max(), integer.step(), 0,
				value -> integer.set((int) Math.round(value)));
			case DoubleSetting number -> number(view, number.get(), number.min(), number.max(), number.step(), decimals(number.step()),
				number::set);
			case ColorSetting color -> this.colors.view(view, color);
			case StringSetting text -> this.texts.text(view, text);
			case EnumSetting<?> choice -> choice(view, choice);
			case KeybindSetting keybind -> keybind(view, keybind);
			case IntRangeSetting range -> range(view, range.low(), range.high(), range.min(), range.max(), range.step(), 0,
				low -> range.set((int) Math.round(low), range.high()), high -> range.set(range.low(), (int) Math.round(high)));
			case DoubleRangeSetting range -> range(view, range.low(), range.high(), range.min(), range.max(), range.step(),
				decimals(range.step()), low -> range.set(low, range.high()), high -> range.set(range.low(), high));
			case EnumSetSetting<?> choices -> choices(view, choices);
			case RegistrySetSetting<?> registry -> this.registries.view(view, registry);
			case StringListSetting list -> this.texts.list(view, list);
			case PositionSetting position -> position(view, position);
			case LanguageSetting language -> this.languages.single(view, language);
			case LanguageListSetting languageList -> this.languages.list(view, languageList);
		}
	}

	public void reset(Setting<?, ?> setting) {
		setting.reset();
		this.texts.forget(setting);
	}

	public void forgetAll() {
		this.capture.cancel();
		this.colors.close();
		this.registries.close();
		this.languages.close();
		this.texts.close();
	}

	@Override
	public void close() {
		forgetAll();
	}

	private static void number(View view, double value, double min, double max, double step, int decimals, DoubleConsumer set) {
		view.row(() -> {
				if (min < max) {
					slider(view, value, min, max, step, set).width(SLIDER_WIDTH);
				}
				valueLabel(view, value, decimals);
			})
			.gap(Rikka.SPACE_SNUG)
			.alignItems(Align.CENTER);
	}

	private static Element slider(View view, double value, double min, double max, double step, DoubleConsumer set) {
		Element slider = view.slider(value).range(min, max).onValue((context, changed) -> set.accept(changed));
		return step > 0.0 ? slider.step(step) : slider;
	}

	private static void range(View view, double low, double high, double min, double max, double step, int decimals,
		DoubleConsumer setLow, DoubleConsumer setHigh) {
		view.column(() -> {
				rangeRow(view, Labels.of("range.low"), low, min, max, step, decimals, setLow);
				rangeRow(view, Labels.of("range.high"), high, min, max, step, decimals, setHigh);
			})
			.gap(Rikka.SPACE_SNUG)
			.fillWidth();
	}

	private static void rangeRow(View view, String label, double value, double min, double max, double step, int decimals,
		DoubleConsumer set) {
		view.row(() -> {
				view.text(label).textStyle(TextRole.CAPTION).tone(TextTone.MUTED).width(RANGE_LABEL_WIDTH);
				if (min < max) {
					slider(view, value, min, max, step, set).grow(1.0F);
				}
				valueLabel(view, value, decimals);
			})
			.gap(Rikka.SPACE_SNUG)
			.alignItems(Align.CENTER)
			.fillWidth();
	}

	private static <E extends Enum<E>> void choice(View view, EnumSetting<E> setting) {
		List<E> constants = setting.constants();
		int selected = constants.indexOf(setting.get());
		if (constants.size() <= SEGMENTED_LIMIT) {
			view.segmented(selected, () -> {
					for (E constant : constants) {
						view.segment(setting.constantName(constant).getString());
					}
				})
				.width(SEGMENT_WIDTH * constants.size())
				.onSelect((context, index) -> setting.set(constants.get(index)));
		} else {
			view.combo(selected, () -> {
					for (E constant : constants) {
						view.option(setting.constantName(constant).getString());
					}
				})
				.width(COMBO_WIDTH)
				.onSelect((context, index) -> setting.set(constants.get(index)));
		}
	}

	private static <E extends Enum<E>> void choices(View view, EnumSetSetting<E> setting) {
		view.row(() -> {
				for (E constant : setting.constants()) {
					CheckState state = setting.contains(constant) ? CheckState.CHECKED : CheckState.UNCHECKED;
					view.checkbox(state, setting.constantName(constant).getString())
						.onToggle((context, on) -> setting.toggle(constant));
				}
			})
			.wrapItems()
			.gap(Rikka.SPACE_NORMAL)
			.fillWidth();
	}

	private void keybind(View view, KeybindSetting setting) {
		boolean capturing = this.capture.captures(setting);
		view.row(() -> {
				view.button(capturing ? Labels.of("key.press") : keyName(setting.get()))
					.buttonStyle(capturing ? ButtonVariant.PRIMARY : ButtonVariant.SECONDARY)
					.minWidth(KEY_WIDTH)
					.onPress(context -> this.capture.toggle(setting));
				if (setting.isBound() && !capturing) {
					view.iconButton("close")
						.tooltip(Labels.of("key.clear"))
						.onPress(context -> setting.unbind());
				}
			})
			.gap(Rikka.SPACE_TIGHT)
			.alignItems(Align.CENTER);
	}

	private static String keyName(InputConstants.Key key) {
		return key.equals(InputConstants.UNKNOWN) ? Labels.of("key.none") : key.getDisplayName().getString();
	}

	private static void position(View view, PositionSetting setting) {
		ScreenPosition position = setting.get();
		Anchor[] anchors = Anchor.values();
		view.row(() -> {
				view.combo(position.anchor().ordinal(), () -> {
						for (Anchor anchor : anchors) {
							view.option(Labels.of("anchor." + anchor.name().toLowerCase(Locale.ROOT)));
						}
					})
					.width(COMBO_WIDTH)
					.onSelect((context, index) -> setting.set(new ScreenPosition(anchors[index], 0.0, 0.0)));
				coordinate(view, "X", position.x(), x -> setting.set(new ScreenPosition(setting.get().anchor(), x, setting.get().y())));
				coordinate(view, "Y", position.y(), y -> setting.set(new ScreenPosition(setting.get().anchor(), setting.get().x(), y)));
			})
			.wrapItems()
			.gap(Rikka.SPACE_NORMAL)
			.alignItems(Align.CENTER)
			.fillWidth();
	}

	private static void coordinate(View view, String axis, double value, DoubleConsumer set) {
		view.row(() -> {
				view.text(axis).textStyle(TextRole.LABEL).tone(TextTone.MUTED);
				view.numberField(value)
					.step(COORDINATE_STEP)
					.decimals(COORDINATE_DECIMALS)
					.width(COORDINATE_WIDTH)
					.onValue((context, changed) -> set.accept(changed));
			})
			.gap(Rikka.SPACE_SNUG)
			.alignItems(Align.CENTER);
	}

	private static int decimals(double step) {
		return step > 0.0 ? Math.max(0, BigDecimal.valueOf(step).stripTrailingZeros().scale()) : DOUBLE_DECIMALS;
	}

	private static void valueLabel(View view, double value, int decimals) {
		view.text(String.format(Locale.ROOT, "%." + decimals + "f", value))
			.textStyle(TextRole.LABEL)
			.tone(TextTone.MUTED)
			.textAlign(TextAlignment.RIGHT)
			.textNoWrap()
			.width(VALUE_WIDTH);
	}
}
