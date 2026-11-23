package sh.rikka.client.menu.settings;

import java.util.Comparator;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import net.minecraft.client.resources.language.I18n;
import net.minecraft.locale.Language;
import net.minecraft.resources.Identifier;
import net.minecraft.util.Util;
import org.jspecify.annotations.Nullable;
import os.rikka.AnchorEdge;
import os.rikka.ButtonVariant;
import os.rikka.CheckState;
import os.rikka.Rikka;
import os.rikka.TextDocument;
import os.rikka.View;
import sh.rikka.client.menu.Labels;
import sh.rikka.client.setting.RegistrySetSetting;

final class RegistryControl implements AutoCloseable {
	private static final float PICKER_WIDTH = 320.0F;
	private static final float LIST_HEIGHT = 300.0F;

	private @Nullable Picking picking;

	void view(View view, RegistrySetSetting<?> setting) {
		String key = "registry:" + setting.translationKey();
		view.button(Labels.of("registry.selected", setting.get().size()))
			.buttonStyle(ButtonVariant.SECONDARY)
			.key(key)
			.onPress(context -> toggle(setting));
		Picking open = this.picking;
		if (open != null && open.setting == setting) {
			view.popover(() -> picker(view, open))
				.anchor(key, AnchorEdge.BELOW_END, 0.0F, Rikka.SPACE_SNUG)
				.onDismiss(context -> close());
		}
	}

	private void toggle(RegistrySetSetting<?> setting) {
		Picking open = this.picking;
		close();
		if (open == null || open.setting != setting) {
			this.picking = new Picking(setting);
		}
	}

	private static void picker(View view, Picking open) {
		List<Entry> shown = open.shown();
		Set<Identifier> selected = open.setting.get();
		view.column(() -> {
				view.textField(open.search)
					.placeholder(Labels.of("search"))
					.autofocus(true)
					.fillWidth()
					.onChange(context -> {
					});
				view.lazyColumn(shown.size(), (item, index, highlighted) -> {
						Entry entry = shown.get(index);
						item.checkbox(selected.contains(entry.id()) ? CheckState.CHECKED : CheckState.UNCHECKED, entry.name())
							.tooltip(entry.id().toString())
							.onToggle((context, on) -> open.flip(entry.id()));
					})
					.height(LIST_HEIGHT)
					.fillWidth();
			})
			.gap(Rikka.SPACE_SNUG)
			.padding(Rikka.SPACE_NORMAL)
			.width(PICKER_WIDTH);
	}

	@Override
	public void close() {
		Picking open = this.picking;
		this.picking = null;
		if (open != null) {
			open.search.close();
		}
	}

	private record Entry(Identifier id, String name) {
	}

	private static final class Picking {
		private final RegistrySetSetting<?> setting;
		private final TextDocument search = new TextDocument("");
		private final List<Entry> entries;

		private Picking(RegistrySetSetting<?> setting) {
			this.setting = setting;
			String kind = setting.registry().key().identifier().getPath();
			this.entries = setting.registry().keySet().stream()
				.map(id -> new Entry(id, name(kind, id)))
				.sorted(Comparator.comparing(Entry::name, String.CASE_INSENSITIVE_ORDER))
				.toList();
		}

		private static String name(String kind, Identifier id) {
			String key = Util.makeDescriptionId(kind, id);
			return Language.getInstance().has(key) ? I18n.get(key) : id.toString();
		}

		private List<Entry> shown() {
			String query = this.search.getText().strip().toLowerCase(Locale.ROOT);
			if (query.isEmpty()) {
				return this.entries;
			}
			return this.entries.stream()
				.filter(entry -> entry.name().toLowerCase(Locale.ROOT).contains(query) || entry.id().toString().contains(query))
				.toList();
		}

		private void flip(Identifier id) {
			Set<Identifier> next = new LinkedHashSet<>(this.setting.get());
			if (!next.remove(id)) {
				next.add(id);
			}
			this.setting.set(next);
		}
	}
}
