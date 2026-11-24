package sh.rikka.client.mods.privacy;

import com.mojang.authlib.GameProfile;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.resources.DefaultPlayerSkin;
import net.minecraft.world.entity.player.PlayerSkin;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.DisconnectEvent;
import sh.rikka.client.events.TickEvent;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.Setting;
import sh.rikka.client.setting.StringSetting;

public final class StreamerModeMod extends Mod {
	private static final int NAME_LENGTH = 32;
	private static volatile @Nullable StreamerModeMod active;

	private final BoolSetting hideName = bool("hide_name", true);
	private final StringSetting name = text("name", "You").maxLength(NAME_LENGTH).visibleWhen(this.hideName::get);
	private final EnumSetting<SkinChoice> skin = choice("skin", SkinChoice.DEFAULT);
	private final BoolSetting slim = bool("slim", false).visibleWhen(() -> this.skin.get().isNamed());
	private final BoolSetting hideOthers = bool("hide_others", false);
	private final StringSetting othersName = text("others_name", "Player").maxLength(NAME_LENGTH).visibleWhen(this.hideOthers::get);
	private final BoolSetting othersSkins = bool("others_skins", false);
	private final Map<UUID, Integer> aliases = new HashMap<>();
	private volatile NameTable names = NameTable.EMPTY;
	private volatile Set<UUID> ownIds = Set.of();
	private volatile @Nullable PlayerSkin ownSkin;
	private int roster;
	private boolean stale = true;

	public StreamerModeMod() {
		super("streamer_mode", Category.UTILITY, "shield_person");
		for (Setting<?, ?> setting : settings().all()) {
			setting.onChange(value -> this.stale = true);
		}
	}

	static @Nullable StreamerModeMod active() {
		return active;
	}

	@Override
	protected void onEnable() {
		this.stale = true;
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
		this.names = NameTable.EMPTY;
		this.ownIds = Set.of();
		this.ownSkin = null;
		this.aliases.clear();
	}

	@EventHandler
	private void onTick(TickEvent.Post event) {
		Minecraft minecraft = Minecraft.getInstance();
		int roster = roster(minecraft);
		if (this.stale || roster != this.roster) {
			this.stale = false;
			this.roster = roster;
			rebuild(minecraft);
		}
	}

	@EventHandler
	private void onDisconnect(DisconnectEvent event) {
		this.aliases.clear();
		this.stale = true;
	}

	NameTable names() {
		return this.names;
	}

	PlayerSkin skin(GameProfile profile, PlayerSkin vanilla) {
		UUID id = profile.id();
		if (this.ownIds.contains(id)) {
			PlayerSkin own = this.ownSkin;
			return own == null ? vanilla : own;
		}
		return this.othersSkins.get() ? DefaultPlayerSkin.get(id) : vanilla;
	}

	private int roster(Minecraft minecraft) {
		LocalPlayer player = minecraft.player;
		int roster = minecraft.getUser().getProfileId().hashCode();
		if (player != null) {
			roster = 31 * roster + player.getUUID().hashCode();
			roster = 31 * roster + player.getGameProfile().name().hashCode();
		}
		if (!this.hideOthers.get()) {
			return roster;
		}
		int others = 0;
		ClientPacketListener connection = minecraft.getConnection();
		if (connection != null) {
			for (PlayerInfo info : connection.getOnlinePlayers()) {
				others += mix(info.getProfile());
			}
		}
		ClientLevel level = minecraft.level;
		if (level != null) {
			for (AbstractClientPlayer other : level.players()) {
				others += mix(other.getGameProfile());
			}
		}
		return 31 * roster + others;
	}

	private static int mix(GameProfile profile) {
		return profile.id().hashCode() * 31 + profile.name().hashCode();
	}

	private void rebuild(Minecraft minecraft) {
		LocalPlayer player = minecraft.player;
		UUID profileId = minecraft.getUser().getProfileId();
		UUID self = player != null ? player.getUUID() : profileId;
		Set<UUID> own = profileId.equals(self) ? Set.of(self) : Set.of(profileId, self);
		Map<String, String> replacements = new HashMap<>();
		if (this.hideOthers.get()) {
			addOthers(minecraft, own, replacements);
		}
		if (this.hideName.get()) {
			addName(replacements, minecraft.getUser().getName(), this.name.get());
			if (player != null) {
				addName(replacements, player.getGameProfile().name(), this.name.get());
			}
		}
		this.ownIds = own;
		this.ownSkin = this.skin.get().skin(self, this.slim.get());
		this.names = NameTable.of(replacements);
	}

	private void addOthers(Minecraft minecraft, Set<UUID> own, Map<String, String> replacements) {
		ClientPacketListener connection = minecraft.getConnection();
		if (connection != null) {
			for (PlayerInfo info : connection.getOnlinePlayers()) {
				addOther(replacements, own, info.getProfile());
			}
		}
		ClientLevel level = minecraft.level;
		if (level != null) {
			for (AbstractClientPlayer other : level.players()) {
				addOther(replacements, own, other.getGameProfile());
			}
		}
	}

	private void addOther(Map<String, String> replacements, Set<UUID> skipped, GameProfile profile) {
		if (skipped.contains(profile.id())) {
			return;
		}
		int alias = this.aliases.computeIfAbsent(profile.id(), id -> this.aliases.size() + 1);
		addName(replacements, profile.name(), this.othersName.get() + alias);
	}

	private static void addName(Map<String, String> replacements, String name, String replacement) {
		if (NameTable.isName(name)) {
			replacements.put(name.toLowerCase(Locale.ROOT), replacement);
		}
	}
}
