package sh.rikka.client.mods.hud;

import java.io.IOException;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Objects;
import net.minecraft.ChatFormatting;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;
import net.minecraft.server.packs.PackMetadataResources;
import net.minecraft.server.packs.repository.BuiltInPackSource;
import net.minecraft.server.packs.repository.Pack;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.resources.IoSupplier;
import org.jspecify.annotations.Nullable;
import os.rikka.Align;
import os.rikka.Element;
import os.rikka.FontWeight;
import os.rikka.Image;
import os.rikka.Rikka;
import os.rikka.View;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.hud.style.HudLook;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.ResourceReloadEvent;
import sh.rikka.client.events.TickEvent;
import sh.rikka.client.mod.HudMod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ScreenPosition;

public final class PackDisplayMod extends HudMod {
	private static final String ICON_FILE = "pack.png";
	private static final float PADDING = 7.0F;
	private static final float GAP = 8.0F;
	private static final float ROW_GAP = 6.0F;
	private static final float ICON = 24.0F;
	private static final float ICON_WITH_DESCRIPTION = 32.0F;
	private static final float ICON_RADIUS = 5.0F;
	private static final float TITLE_SIZE = 13.0F;
	private static final float DESCRIPTION_SIZE = 11.0F;
	private static final float MAX_TEXT_WIDTH = 220.0F;

	private final BoolSetting all = bool("all", false);
	private final BoolSetting icon = bool("icon", true);
	private final BoolSetting description = bool("description", false);
	private final BoolSetting background = bool("background", true);
	private List<Entry> entries = List.of();
	private boolean stale;

	public PackDisplayMod(ScreenPosition defaultPosition) {
		super("pack_display", defaultPosition, "folder_zip");
	}

	@Override
	protected void onEnable() {
		this.stale = true;
	}

	@Override
	protected void onDisable() {
		replace(List.of());
	}

	@EventHandler
	private void onReload(ResourceReloadEvent event) {
		refresh();
	}

	@EventHandler
	private void onTick(TickEvent.Pre event) {
		if (this.stale) {
			refresh();
		}
	}

	@Override
	public float width() {
		float text = 0.0F;
		for (Entry entry : shown()) {
			text = Math.max(text, Math.min(MAX_TEXT_WIDTH, textWidth(entry)));
		}
		float icon = this.icon.get() ? iconSize() + GAP : 0.0F;
		return PADDING * 2.0F + icon + (float) Math.ceil(text);
	}

	@Override
	public float height() {
		int rows = Math.max(1, shown().size());
		return PADDING * 2.0F + rows * rowHeight() + (rows - 1) * ROW_GAP;
	}

	@Override
	protected void view(View view, HudLook look) {
		Element panel = view.box(() -> view.column(() -> {
			for (Entry entry : shown()) {
				row(view, look, entry);
			}
		}).fillWidth().fillHeight().gap(ROW_GAP)).fillWidth().fillHeight().padding(PADDING);
		if (this.background.get()) {
			look.panel(panel);
		}
	}

	private void row(View view, HudLook look, Entry entry) {
		view.row(() -> {
			Image image = entry.icon();
			if (this.icon.get() && image != null) {
				view.box(() -> view.imageView(image).fillWidth().fillHeight())
					.width(iconSize())
					.height(iconSize())
					.shrink(0.0F)
					.cornerRadius(ICON_RADIUS)
					.clipContent();
			}
			view.box(() -> view.column(() -> {
				look.text(view.text(entry.title()).textSize(TITLE_SIZE).textWeight(FontWeight.MEDIUM).textNoWrap());
				if (this.description.get() && !entry.description().isEmpty()) {
					look.mutedText(view.text(entry.description()).textSize(DESCRIPTION_SIZE).textNoWrap());
				}
			}).fillWidth()).grow(1.0F).clipContent();
		}).height(rowHeight()).gap(GAP).alignItems(Align.CENTER);
	}

	private List<Entry> shown() {
		List<Entry> entries = this.entries;
		return this.all.get() || entries.size() <= 1 ? entries : entries.subList(0, 1);
	}

	private float textWidth(Entry entry) {
		return this.description.get() ? Math.max(entry.titleWidth(), entry.descriptionWidth()) : entry.titleWidth();
	}

	private float iconSize() {
		return this.description.get() ? ICON_WITH_DESCRIPTION : ICON;
	}

	private float rowHeight() {
		return iconSize();
	}

	private void refresh() {
		this.stale = false;
		PackRepository repository = Minecraft.getInstance().getResourcePackRepository();
		List<Pack> chosen = chosen(repository.getSelectedPacks());
		if (chosen.isEmpty()) {
			Pack vanilla = repository.getPack(BuiltInPackSource.VANILLA_ID);
			if (vanilla != null) {
				chosen = List.of(vanilla);
			}
		}
		List<Entry> entries = new ArrayList<>(chosen.size());
		for (Pack pack : chosen) {
			entries.add(Entry.of(plain(pack.getTitle()), firstLine(plain(pack.getDescription())), readIcon(pack)));
		}
		replace(List.copyOf(entries));
	}

	private void replace(List<Entry> entries) {
		List<Entry> previous = this.entries;
		this.entries = entries;
		for (Entry entry : previous) {
			Image image = entry.icon();
			if (image != null) {
				image.close();
			}
		}
	}

	private static List<Pack> chosen(Collection<Pack> selected) {
		List<Pack> chosen = new ArrayList<>();
		for (Pack pack : selected) {
			if (!pack.isRequired()) {
				chosen.addFirst(pack);
			}
		}
		return chosen;
	}

	private static @Nullable Image readIcon(Pack pack) {
		try (PackMetadataResources resources = pack.openMetadata()) {
			IoSupplier<InputStream> icon = resources.getRootResource(ICON_FILE);
			if (icon == null) {
				return null;
			}
			try (InputStream stream = icon.get()) {
				return Image.fromEncoded(stream.readAllBytes());
			}
		} catch (IOException failure) {
			RikkaClient.LOGGER.warn("failed to read the icon of pack {}: {}", pack.getId(), failure.toString());
			return null;
		}
	}

	private static String plain(Component component) {
		return Objects.requireNonNullElse(ChatFormatting.stripFormatting(component.getString()), "");
	}

	private static String firstLine(String text) {
		int end = text.indexOf('\n');
		return (end < 0 ? text : text.substring(0, end)).strip();
	}

	private record Entry(String title, String description, @Nullable Image icon, float titleWidth, float descriptionWidth) {
		static Entry of(String title, String description, @Nullable Image icon) {
			return new Entry(
				title,
				description,
				icon,
				Rikka.textWidth(title, TITLE_SIZE),
				description.isEmpty() ? 0.0F : Rikka.textWidth(description, DESCRIPTION_SIZE)
			);
		}
	}
}
