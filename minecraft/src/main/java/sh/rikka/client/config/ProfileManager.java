package sh.rikka.client.config;

import com.google.gson.JsonObject;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.time.Duration;
import java.util.List;
import java.util.concurrent.ScheduledThreadPoolExecutor;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import sh.rikka.client.event.EventBus;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.mod.ModManager;
import sh.rikka.client.mod.ModToggleEvent;
import sh.rikka.client.setting.Setting;
import sh.rikka.client.setting.Settings;

public final class ProfileManager implements AutoCloseable {
	public static final String DEFAULT_PROFILE = "default";
	public static final int MAX_NAME_LENGTH = 32;

	private static final Logger LOGGER = LoggerFactory.getLogger(ProfileManager.class);
	private static final Duration SAVE_DELAY = Duration.ofMillis(500);
	private static final String SAVER_THREAD = "rikka-client-profiles";
	private static final String PREFERENCES = "preferences";

	private final ProfileStore store;
	private final List<Mod> mods;
	private final Settings preferences;
	private final JsonObject preferencesDocument;
	private final ScheduledThreadPoolExecutor saver;
	private final AtomicBoolean saveScheduled = new AtomicBoolean();
	private final AtomicBoolean profileDirty = new AtomicBoolean();
	private final AtomicBoolean preferencesDirty = new AtomicBoolean();
	private volatile boolean applying;
	private volatile boolean closed;
	private String active;
	private JsonObject document;

	private ProfileManager(ProfileStore store, List<Mod> mods, Settings preferences, JsonObject preferencesDocument, String active) throws IOException {
		this.store = store;
		this.mods = mods;
		this.preferences = preferences;
		this.preferencesDocument = preferencesDocument;
		this.active = active;
		this.document = store.read(active);
		this.saver = new ScheduledThreadPoolExecutor(1, task -> {
			Thread thread = new Thread(task, SAVER_THREAD);
			thread.setDaemon(true);
			return thread;
		});
		this.saver.setExecuteExistingDelayedTasksAfterShutdownPolicy(false);
	}

	public static ProfileManager open(Path directory, ModManager mods, Preferences preferences, EventBus bus) {
		if (!mods.isLocked()) {
			throw new IllegalStateException("register and lock every mod before opening profiles");
		}
		ProfileStore store = new ProfileStore(directory);
		try {
			String active = store.readActive().map(name -> {
				if (isValidName(name)) {
					return name;
				}
				LOGGER.warn("ignoring invalid active profile name {}", name);
				return DEFAULT_PROFILE;
			}).orElse(DEFAULT_PROFILE);
			JsonObject preferencesDocument = store.readPreferences();
			ProfileCodec.decode(preferences.settings(), preferencesDocument, PREFERENCES);
			ProfileManager profiles = new ProfileManager(store, mods.all(), preferences.settings(), preferencesDocument, active);
			profiles.apply();
			profiles.writeCurrent();
			store.writeState(active, profiles.encodedPreferences());
			for (Mod mod : mods.all()) {
				profiles.track(mod.controls(), profiles.profileDirty);
				profiles.track(mod.settings(), profiles.profileDirty);
			}
			profiles.track(preferences.settings(), profiles.preferencesDirty);
			bus.subscribe(profiles);
			return profiles;
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to open profiles in " + directory, failure);
		}
	}

	public static boolean isValidName(String name) {
		return ProfileStore.isValidName(name);
	}

	public synchronized String active() {
		return this.active;
	}

	public List<String> profiles() {
		try {
			return this.store.list();
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to list profiles", failure);
		}
	}

	public synchronized void switchTo(String name) {
		requireValidName(name);
		if (name.equals(this.active)) {
			return;
		}
		if (!this.store.exists(name)) {
			throw new IllegalArgumentException("no profile named " + name);
		}
		try {
			writeCurrent();
			JsonObject next = this.store.read(name);
			this.store.writeState(name, encodedPreferences());
			this.active = name;
			this.document = next;
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to switch to profile " + name, failure);
		}
		apply();
	}

	public synchronized void create(String name) {
		requireValidName(name);
		if (this.store.exists(name)) {
			throw new IllegalArgumentException("profile " + name + " already exists");
		}
		try {
			this.store.write(name, ProfileCodec.encode(this.document, this.mods));
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to create profile " + name, failure);
		}
	}

	public synchronized void delete(String name) {
		requireValidName(name);
		if (name.equals(this.active)) {
			throw new IllegalStateException("cannot delete the active profile " + name);
		}
		try {
			this.store.delete(name);
		} catch (NoSuchFileException failure) {
			throw new IllegalArgumentException("no profile named " + name, failure);
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to delete profile " + name, failure);
		}
	}

	@Override
	public void close() {
		this.closed = true;
		this.saver.shutdown();
		save();
	}

	@EventHandler
	private void onModToggle(ModToggleEvent event) {
		if (ProfileCodec.persistsEnabled(event.mod())) {
			markDirty(this.profileDirty);
		}
	}

	private void track(Settings settings, AtomicBoolean dirty) {
		for (Setting<?, ?> setting : settings.all()) {
			setting.onChange(value -> markDirty(dirty));
		}
	}

	private void markDirty(AtomicBoolean dirty) {
		if (this.applying || this.closed) {
			return;
		}
		dirty.set(true);
		if (this.saveScheduled.compareAndSet(false, true)) {
			this.saver.schedule(this::scheduledSave, SAVE_DELAY.toMillis(), TimeUnit.MILLISECONDS);
		}
	}

	private void scheduledSave() {
		this.saveScheduled.set(false);
		try {
			save();
		} catch (RuntimeException failure) {
			LOGGER.error("failed to save profile {}", active(), failure);
		}
	}

	private synchronized void save() {
		try {
			if (this.profileDirty.getAndSet(false)) {
				writeCurrent();
			}
		} catch (IOException failure) {
			this.profileDirty.set(true);
			LOGGER.error("failed to save profile {}", this.active, failure);
		}
		try {
			if (this.preferencesDirty.getAndSet(false)) {
				this.store.writeState(this.active, encodedPreferences());
			}
		} catch (IOException failure) {
			this.preferencesDirty.set(true);
			LOGGER.error("failed to save preferences", failure);
		}
	}

	private void writeCurrent() throws IOException {
		this.document = ProfileCodec.encode(this.document, this.mods);
		this.store.write(this.active, this.document);
	}

	private JsonObject encodedPreferences() {
		ProfileCodec.encode(this.preferences, this.preferencesDocument);
		return this.preferencesDocument;
	}

	private void apply() {
		this.applying = true;
		try {
			ProfileCodec.apply(this.document, this.mods);
		} finally {
			this.applying = false;
		}
	}

	private static void requireValidName(String name) {
		if (!isValidName(name)) {
			throw new IllegalArgumentException("invalid profile name: " + name);
		}
	}
}
