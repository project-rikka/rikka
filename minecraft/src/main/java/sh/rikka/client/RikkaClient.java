package sh.rikka.client;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import sh.rikka.client.config.Preferences;
import sh.rikka.client.config.ProfileManager;
import sh.rikka.client.event.Event;
import sh.rikka.client.event.EventBus;
import sh.rikka.client.events.CombatTracker;
import sh.rikka.client.events.KeyPresses;
import sh.rikka.client.menu.MenuKeys;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.mod.ModKeybinds;
import sh.rikka.client.mod.ModManager;
import sh.rikka.client.mods.BuiltinMods;
import sh.rikka.client.music.MusicKeys;
import sh.rikka.client.music.MusicPlayer;
import sh.rikka.client.target.TargetTracker;
import sh.rikka.client.ui.HudOverlays;
import sh.rikka.client.ui.RikkaUi;

public final class RikkaClient implements ClientModInitializer {
	public static final String MOD_ID = "rikka";
	public static final String NAME = "Rikka Client";
	public static final Logger LOGGER = LoggerFactory.getLogger(MOD_ID);

	private static final String MUSIC_FOLDER = "music";
	private static final EventBus EVENTS = new EventBus(RikkaClient::onListenerFailure);
	private static final TargetTracker TARGETS = new TargetTracker();
	private static volatile @Nullable Services services;

	@Override
	public void onInitializeClient() {
		FabricLoader loader = FabricLoader.getInstance();
		ModManager mods = new ModManager(EVENTS);
		BuiltinMods.register(mods);
		mods.lock();
		Preferences preferences = new Preferences();
		ProfileManager profiles = ProfileManager.open(loader.getConfigDir().resolve(MOD_ID), mods, preferences, EVENTS);
		MusicPlayer music = MusicPlayer.open(loader.getGameDir().resolve(MOD_ID).resolve(MUSIC_FOLDER), preferences, EVENTS, Minecraft.getInstance());
		EVENTS.subscribe(new ModKeybinds(mods));
		EVENTS.subscribe(new CombatTracker(EVENTS));
		EVENTS.subscribe(TARGETS);
		EVENTS.subscribe(new HudOverlays(mods));
		EVENTS.subscribe(new KeyPresses(new MenuKeys(preferences)));
		EVENTS.subscribe(new KeyPresses(new MusicKeys(preferences, music)));
		services = new Services(mods, preferences, profiles, music);
		LOGGER.info("{} initialized with profile {}", NAME, profiles.active());
	}

	public static EventBus events() {
		return EVENTS;
	}

	public static TargetTracker targets() {
		return TARGETS;
	}

	public static ModManager mods() {
		return requireServices().mods();
	}

	public static Preferences preferences() {
		return requireServices().preferences();
	}

	public static ProfileManager profiles() {
		return requireServices().profiles();
	}

	public static MusicPlayer music() {
		return requireServices().music();
	}

	public static void shutdown() {
		RikkaUi.shutdown();
		Services current = services;
		if (current != null) {
			current.music().close();
			current.profiles().close();
		}
	}

	private static Services requireServices() {
		Services current = services;
		if (current == null) {
			throw new IllegalStateException(NAME + " is not initialized");
		}
		return current;
	}

	private static void onListenerFailure(Object owner, Event event, Exception failure) {
		if (!(owner instanceof Mod mod)) {
			throw new IllegalStateException("listener failed on " + event.getClass().getName(), failure);
		}
		LOGGER.error("{} failed on {} and is disabled", mod.id(), event.getClass().getName(), failure);
		Minecraft minecraft = Minecraft.getInstance();
		if (minecraft.isSameThread()) {
			mod.setEnabled(false);
		} else {
			minecraft.execute(() -> mod.setEnabled(false));
		}
	}

	private record Services(ModManager mods, Preferences preferences, ProfileManager profiles, MusicPlayer music) {
	}
}
