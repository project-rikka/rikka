package sh.rikka.client.menu.settings;

import java.text.Collator;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.function.Consumer;
import java.util.function.Supplier;
import org.jspecify.annotations.Nullable;
import os.rikka.Align;
import os.rikka.AnchorEdge;
import os.rikka.ButtonVariant;
import os.rikka.Element;
import os.rikka.Rikka;
import os.rikka.TextDocument;
import os.rikka.View;
import sh.rikka.client.menu.Labels;
import sh.rikka.client.setting.LanguageListSetting;
import sh.rikka.client.setting.LanguageSetting;
import sh.rikka.client.setting.Setting;
import sh.rikka.client.translate.Languages;

final class LanguageControl implements AutoCloseable {
	private static final float PICKER_WIDTH = 320.0F;
	private static final float LIST_HEIGHT = 300.0F;

	private @Nullable Picking picking;

	void single(View view, LanguageSetting setting) {
		Locale display = LanguageSetting.gameLocale();
		String key = "language:" + setting.translationKey();
		view.button(label(setting.get(), display))
			.buttonStyle(ButtonVariant.SECONDARY)
			.fillWidth()
			.key(key)
			.onPress(context -> toggle(setting, () -> entries(setting, display), setting::set));
		popover(view, setting, key, setting.get());
	}

	void list(View view, LanguageListSetting setting) {
		Locale display = LanguageSetting.gameLocale();
		String key = "languages:" + setting.translationKey();
		List<String> chosen = setting.get();
		view.column(() -> {
				for (String tag : chosen) {
					view.row(() -> {
							view.text(Languages.name(tag, display)).grow(1.0F).shrink(1.0F);
							view.iconButton("close")
								.tooltip(Labels.of("list.remove"))
								.onPress(context -> setting.remove(tag));
						})
						.alignItems(Align.CENTER)
						.fillWidth()
						.key("language:" + tag);
				}
				view.button(Labels.of("language.add"))
					.buttonStyle(ButtonVariant.SECONDARY)
					.key(key)
					.onPress(context -> toggle(setting, () -> entries(setting, display), setting::add));
				popover(view, setting, key, null);
			})
			.gap(Rikka.SPACE_TIGHT)
			.fillWidth();
	}

	private void popover(View view, Setting<?, ?> setting, String key, @Nullable String selected) {
		Picking open = this.picking;
		if (open != null && open.setting == setting) {
			view.popover(() -> picker(view, open, selected))
				.anchor(key, AnchorEdge.BELOW_END, 0.0F, Rikka.SPACE_SNUG)
				.onDismiss(context -> close());
		}
	}

	private static int selectionOf(List<Entry> shown, @Nullable String selected) {
		for (int index = 0; index < shown.size(); index++) {
			if (shown.get(index).tag().equals(selected)) {
				return index;
			}
		}
		return Rikka.NONE;
	}

	private void picker(View view, Picking open, @Nullable String selected) {
		List<Entry> shown = open.shown();
		int selection = selectionOf(shown, selected);
		view.column(() -> {
				view.textField(open.search)
					.placeholder(Labels.of("search"))
					.autofocus(true)
					.fillWidth()
					.onChange(context -> {
					});
				view.lazyList(shown.size(), (item, index, highlighted) -> {
						Entry entry = shown.get(index);
						Element row = item.listItem(entry.name());
						if (!entry.detail().equals(entry.name())) {
							row.secondaryText(entry.detail());
						}
					})
					.selection(selection)
					.onSelect((context, index) -> {
						open.choose.accept(shown.get(index).tag());
						close();
					})
					.height(LIST_HEIGHT)
					.fillWidth();
			})
			.gap(Rikka.SPACE_SNUG)
			.padding(Rikka.SPACE_NORMAL)
			.width(PICKER_WIDTH);
	}

	private void toggle(Setting<?, ?> setting, Supplier<List<Entry>> entries, Consumer<String> choose) {
		Picking open = this.picking;
		close();
		if (open == null || open.setting != setting) {
			this.picking = new Picking(setting, entries.get(), choose);
		}
	}

	private static List<Entry> entries(LanguageSetting setting, Locale display) {
		List<Entry> entries = new ArrayList<>();
		if (setting.offersGame()) {
			entries.add(new Entry(LanguageSetting.GAME, Labels.of("language.game"), Languages.name(LanguageSetting.gameLanguage(), display)));
		}
		entries.addAll(languages(display, List.of()));
		return entries;
	}

	private static List<Entry> entries(LanguageListSetting setting, Locale display) {
		return languages(display, setting.get());
	}

	private static List<Entry> languages(Locale display, List<String> excluded) {
		return Languages.targets().stream()
			.filter(tag -> !excluded.contains(tag))
			.map(tag -> new Entry(tag, Languages.name(tag, display), Languages.nativeName(tag)))
			.sorted(Comparator.comparing(Entry::name, Collator.getInstance(display)))
			.toList();
	}

	private static String label(String tag, Locale display) {
		if (LanguageSetting.GAME.equals(tag)) {
			return Labels.of("language.game_named", Languages.name(LanguageSetting.gameLanguage(), display));
		}
		return Languages.name(tag, display);
	}

	@Override
	public void close() {
		Picking open = this.picking;
		this.picking = null;
		if (open != null) {
			open.search.close();
		}
	}

	private record Entry(String tag, String name, String detail) {
		private boolean matches(String query) {
			return this.name.toLowerCase(Locale.ROOT).contains(query)
				|| this.detail.toLowerCase(Locale.ROOT).contains(query)
				|| this.tag.toLowerCase(Locale.ROOT).startsWith(query);
		}
	}

	private static final class Picking {
		private final Setting<?, ?> setting;
		private final List<Entry> entries;
		private final Consumer<String> choose;
		private final TextDocument search = new TextDocument("");

		private Picking(Setting<?, ?> setting, List<Entry> entries, Consumer<String> choose) {
			this.setting = setting;
			this.entries = entries;
			this.choose = choose;
		}

		private List<Entry> shown() {
			String query = this.search.getText().strip().toLowerCase(Locale.ROOT);
			if (query.isEmpty()) {
				return this.entries;
			}
			return this.entries.stream().filter(entry -> entry.matches(query)).toList();
		}
	}
}
