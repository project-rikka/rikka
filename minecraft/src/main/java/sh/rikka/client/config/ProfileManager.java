package sh.rikka.client.config;

import com.google.gson.JsonObject;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.time.Duration;
import java.util.HashSet;
import java.util.List;
import java.util.Optional;
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
	private final ServerIndex servers = new ServerIndex();
	private final ScheduledThreadPoolExecutor saver;
	private final AtomicBoolean saveScheduled = new AtomicBoolean();
	private final AtomicBoolean profileDirty = new AtomicBoolean();
	private final AtomicBoolean preferencesDirty = new AtomicBoolean();
	private volatile boolean applying;
	private volatile boolean closed;
	private String selected;
	private String active;
	private JsonObject document;

	private ProfileManager(ProfileStore store, List<Mod> mods, Settings preferences, JsonObject preferencesDocument, String selected) throws IOException {
		this.store = store;
		this.mods = mods;
		this.preferences = preferences;
		this.preferencesDocument = preferencesDocument;
		this.selected = selected;
		this.active = selected;
		this.document = store.read(selected);
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
			String selected = store.readActive().map(name -> {
				if (isValidName(name)) {
					return name;
				}
				LOGGER.warn("ignoring invalid active profile name {}", name);
				return DEFAULT_PROFILE;
			}).orElse(DEFAULT_PROFILE);
			JsonObject preferencesDocument = store.readPreferences();
			ProfileCodec.decode(preferences.settings(), preferencesDocument, PREFERENCES);
			ProfileManager profiles = new ProfileManager(store, mods.all(), preferences.settings(), preferencesDocument, selected);
			profiles.apply();
			profiles.writeCurrent();
			store.writeState(selected, profiles.encodedPreferences());
			profiles.indexServers();
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

	public synchronized String selected() {
		return this.selected;
	}

	public synchronized boolean isOverridden() {
		return !this.active.equals(this.selected);
	}

	public List<String> profiles() {
		try {
			return this.store.list();
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to list profiles", failure);
		}
	}

	public synchronized void switchTo(String name) {
		requireExisting(name);
		if (name.equals(this.selected) && name.equals(this.active)) {
			return;
		}
		try {
			if (name.equals(this.active)) {
				this.store.writeState(name, encodedPreferences());
				this.selected = name;
				return;
			}
			JsonObject next = replaceWith(name);
			this.store.writeState(name, encodedPreferences());
			this.selected = name;
			enter(name, next);
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to switch to profile " + name, failure);
		}
	}

	public synchronized boolean override(String name) {
		requireExisting(name);
		if (name.equals(this.active)) {
			return false;
		}
		try {
			enter(name, replaceWith(name));
			return true;
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to load profile " + name, failure);
		}
	}

	public synchronized Optional<String> clearOverride() {
		if (!isOverridden()) {
			return Optional.empty();
		}
		try {
			enter(this.selected, replaceWith(this.selected));
			return Optional.of(this.selected);
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to restore profile " + this.selected, failure);
		}
	}

	public synchronized void create(String name, List<ServerPattern> patterns) {
		requireValidName(name);
		if (this.store.exists(name)) {
			throw new IllegalArgumentException("profile " + name + " already exists");
		}
		requireAvailable(name, patterns);
		try {
			this.store.write(name, ProfileCodec.withServers(ProfileCodec.encode(this.document, this.mods), patterns));
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to create profile " + name, failure);
		}
		this.servers.put(name, patterns);
	}

	public synchronized void delete(String name) {
		requireValidName(name);
		if (name.equals(this.active) || name.equals(this.selected)) {
			throw new IllegalStateException("cannot delete the profile in use " + name);
		}
		try {
			this.store.delete(name);
		} catch (NoSuchFileException failure) {
			throw new IllegalArgumentException("no profile named " + name, failure);
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to delete profile " + name, failure);
		}
		this.servers.remove(name);
	}

	public synchronized List<ServerPattern> servers(String profile) {
		return this.servers.get(profile);
	}

	public synchronized void setServers(String profile, List<ServerPattern> patterns) {
		requireExisting(profile);
		requireAvailable(profile, patterns);
		try {
			if (profile.equals(this.active)) {
				this.document = ProfileCodec.withServers(this.document, patterns);
				writeCurrent();
			} else {
				this.store.write(profile, ProfileCodec.withServers(this.store.read(profile), patterns));
			}
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to save servers of profile " + profile, failure);
		}
		this.servers.put(profile, patterns);
	}

	public synchronized Optional<String> serverOwner(ServerPattern pattern) {
		return this.servers.owner(pattern);
	}

	public synchronized Optional<String> profileFor(String host, int port) {
		return this.servers.match(host, port);
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

	private void indexServers() throws IOException {
		for (String profile : this.store.list()) {
			JsonObject profileDocument = profile.equals(this.active) ? this.document : this.store.read(profile);
			List<ServerPattern> patterns = ProfileCodec.servers(profileDocument, profile).stream().filter(pattern -> {
				Optional<String> owner = this.servers.owner(pattern);
				owner.ifPresent(winner -> LOGGER.warn("ignoring server {} of profile {}: profile {} lists it", pattern, profile, winner));
				return owner.isEmpty();
			}).toList();
			this.servers.put(profile, patterns);
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
				this.store.writeState(this.selected, encodedPreferences());
			}
		} catch (IOException failure) {
			this.preferencesDirty.set(true);
			LOGGER.error("failed to save preferences", failure);
		}
	}

	private JsonObject replaceWith(String name) throws IOException {
		writeCurrent();
		return this.store.read(name);
	}

	private void enter(String name, JsonObject next) {
		this.active = name;
		this.document = next;
		apply();
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

	private void requireExisting(String name) {
		requireValidName(name);
		if (!this.store.exists(name)) {
			throw new IllegalArgumentException("no profile named " + name);
		}
	}

	private void requireAvailable(String profile, List<ServerPattern> patterns) {
		if (new HashSet<>(patterns).size() != patterns.size()) {
			throw new IllegalArgumentException("duplicate server in profile " + profile);
		}
		for (ServerPattern pattern : patterns) {
			this.servers.owner(pattern).filter(owner -> !owner.equals(profile)).ifPresent(owner -> {
				throw new IllegalArgumentException("server " + pattern + " already belongs to profile " + owner);
			});
		}
	}

	private static void requireValidName(String name) {
		if (!isValidName(name)) {
			throw new IllegalArgumentException("invalid profile name: " + name);
		}
	}
}
