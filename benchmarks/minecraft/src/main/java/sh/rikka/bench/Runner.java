package sh.rikka.bench;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonObject;
import com.mojang.renderpearl.api.commands.RenderPass;
import java.io.IOException;
import java.lang.management.ManagementFactory;
import java.nio.file.Files;
import java.util.ArrayDeque;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.function.Consumer;
import java.util.function.Function;
import java.util.function.Supplier;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.inventory.InventoryScreen;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.chunk.CompiledSectionMesh;
import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.Difficulty;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.LevelSettings;
import net.minecraft.world.level.WorldDataConfiguration;
import net.minecraft.world.level.levelgen.WorldOptions;
import net.minecraft.world.level.levelgen.presets.WorldPresets;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

final class Runner {
	private static final Logger LOGGER = LoggerFactory.getLogger("rikka-bench");
	private static final String REPORT = "report.json";
	private static final long STILL_GAME_TIME = 1000L;
	private static final long SECOND = 1_000_000_000L;
	private static final long CHUNKS_MINIMUM = 2 * SECOND;
	private static final long CHUNKS_STABLE = 3 * SECOND;
	private static final long CHUNKS_IDLE = SECOND;
	private static final long CHUNKS_TIMEOUT = 900 * SECOND;
	private static final String HAND_STILL = "hand";
	private static final String INVENTORY_STILL = "inventory";
	private static final long FADE = 2 * SECOND;
	private static final int FLIGHT_WARMUP_TICKS = 40;
	private static final int LAP_TICKS = 600;
	private static final int HOVER_TICKS = 300;
	private static final double CRUISE_ALTITUDE = 60.0;
	private static final double CRUISE_BLOCKS_PER_TICK = 2.0;
	private static final double ORBIT_RADIUS = 96.0;
	private static final double ARRIVE_DISTANCE = 1600.0;

	private final Settings settings;
	private final Minecraft minecraft = Minecraft.getInstance();
	private final ArrayDeque<Step> steps = new ArrayDeque<>();
	private final FrameLog frames = new FrameLog();
	private final GpuTimer gpuTimer = new GpuTimer(this.frames::gpuSpan);
	private final Stills stills;
	private final JsonObject report = new JsonObject();
	private final JsonObject stillResults = new JsonObject();
	private final JsonObject flightResults = new JsonObject();
	private boolean still = true;
	private @Nullable Flight flight;
	private int flightTick;
	private int ground;
	private long mark;

	Runner(Settings settings) {
		this.settings = settings;
		this.stills = new Stills(settings.output(), settings.reference());
		this.report.addProperty("label", settings.label());
		this.report.addProperty("preset", settings.preset().name());
		this.report.add("stills", this.stillResults);
		this.report.add("flights", this.flightResults);
		this.script();
	}

	boolean animatesTextures() {
		return !this.still;
	}

	void gpuBegin(GpuSpan span, @Nullable RenderPass pass) {
		this.gpuTimer.begin(span, pass);
	}

	void gpuEnd(GpuSpan span, @Nullable RenderPass pass) {
		this.gpuTimer.end(span, pass);
		if (span == GpuSpan.FRAME) {
			this.gpuTimer.finishFrame();
		}
	}

	void waited(Wait wait, long nanos) {
		this.frames.waited(wait, nanos);
	}

	void frame() {
		this.frames.frame();
		try {
			ClientLevel level = this.minecraft.level;
			if (this.still && level != null) {
				level.setTimeFromServer(STILL_GAME_TIME);
			}
			while (!this.steps.isEmpty() && this.steps.peek().advance()) {
				this.steps.poll();
			}
		} catch (RuntimeException | IOException error) {
			LOGGER.error("benchmark failed", error);
			Runtime.getRuntime().halt(1);
		}
	}

	void tickStarting() {
		this.frames.tickStarted();
	}

	void tick() {
		this.frames.tickFinished();
		Flight flown = this.flight;
		LocalPlayer player = this.minecraft.player;
		if (flown == null || player == null) {
			return;
		}
		if (this.flightTick == 0) {
			LOGGER.info("flight {} started", flown.name());
			this.frames.start(flown.name());
		}
		Pose pose = flown.path().at(Math.max(this.flightTick, 0));
		player.setPos(pose.x(), pose.y(), pose.z());
		player.setYRot(pose.yaw());
		player.setXRot(pose.pitch());
		this.flightTick++;
	}

	private void script() {
		Supplier<Pose> overview = () -> new Pose(World.ORIGIN + 0.5, this.ground + 48.0, World.ORIGIN + 0.5, -45.0F, 20.0F);
		Supplier<Pose> street = () -> new Pose(World.ORIGIN + 0.5, this.ground + 1.0, World.ORIGIN + 0.5, 135.0F, 0.0F);
		Supplier<Pose> down = () -> new Pose(World.ORIGIN + 0.5, this.ground + 90.0, World.ORIGIN + 0.5, 0.0F, 70.0F);
		Supplier<Pose> crowd = () -> new Pose(
			World.CROWD_ORIGIN - 4.0,
			this.ground + World.CROWD_HEIGHT + 7.0,
			World.CROWD_ORIGIN + World.CROWD_SIDE * World.CROWD_SPACING / 2.0,
			-90.0F,
			20.0F
		);
		Supplier<Pose> wardrobe = () -> new Pose(World.CROWD_ORIGIN - 1.0, this.ground + World.CROWD_HEIGHT + 4.0, Wardrobe.centerZ(), -90.0F, 22.0F);
		Supplier<Pose> dresser = () -> new Pose(World.CROWD_ORIGIN + 1.5, this.ground + World.CROWD_HEIGHT + 1.0, Wardrobe.centerZ(), -90.0F, 10.0F);
		Supplier<Pose> vault = () -> Vault.view(this.ground);
		this.steps.add(this::titleShown);
		this.steps.add(this::openWorld);
		this.steps.add(this::joined);
		this.steps.add(this.onServer(server -> {
			this.ground = World.prepare(server);
			World.freeze(server, true);
		}));
		this.steps.add(() -> this.hideHud(true));
		this.still("overview", overview, World::clearEntities);
		this.still("street", street, World::clearEntities);
		this.still("down", down, World::clearEntities);
		this.still("crowd", crowd, this::rebuildCrowd);
		this.still("wardrobe", wardrobe, this::rebuildWardrobe);
		this.interfaceStills(dresser);
		this.still("vault", vault, this::rebuildVault);
		this.steps.add(this.onServer(server -> {
			World.pacify(server);
			World.freeze(server, false);
		}));
		this.steps.add(() -> {
			this.still = false;
			return this.hideHud(false);
		});
		this.fly(
			"orbit_cold",
			name -> Flight.orbit(name, LAP_TICKS, World.ORIGIN, this.ground + 40.0, World.ORIGIN, ORBIT_RADIUS, 15.0F),
			World::clearEntities
		);
		this.fly(
			"orbit_warm",
			name -> Flight.orbit(name, LAP_TICKS, World.ORIGIN, this.ground + 40.0, World.ORIGIN, ORBIT_RADIUS, 15.0F),
			World::clearEntities
		);
		this.fly("spin", name -> Flight.spin(name, HOVER_TICKS, street.get()), World::clearEntities);
		this.fly(
			"cruise",
			name -> Flight.line(name, LAP_TICKS, overview.get().raised(CRUISE_ALTITUDE - 48.0), CRUISE_BLOCKS_PER_TICK),
			World::clearEntities
		);
		this.fly("crowd", name -> Flight.hover(name, HOVER_TICKS, crowd.get()), this::rebuildCrowd);
		this.fly("gear", name -> Flight.hover(name, HOVER_TICKS, crowd.get()), this::rebuildGear);
		this.arrive("arrive", overview, () -> overview.get().moved(ARRIVE_DISTANCE));
		this.fly("vault", name -> Flight.hover(name, HOVER_TICKS, vault.get()), this::rebuildVault);
		this.steps.add(this::finish);
	}

	private void rebuildCrowd(MinecraftServer server) {
		World.clearEntities(server);
		World.buildCrowd(server, this.ground + World.CROWD_HEIGHT);
	}

	private void rebuildGear(MinecraftServer server) {
		World.clearEntities(server);
		World.buildGear(server, this.ground + World.CROWD_HEIGHT);
	}

	private void rebuildVault(MinecraftServer server) {
		World.clearEntities(server);
		Vault.build(server, this.ground);
	}

	private void rebuildWardrobe(MinecraftServer server) {
		World.clearEntities(server);
		World.buildWardrobe(server, this.ground + World.CROWD_HEIGHT);
	}

	private void interfaceStills(Supplier<Pose> pose) {
		if (!this.settings.includes(HAND_STILL) && !this.settings.includes(INVENTORY_STILL)) {
			return;
		}
		this.steps.add(this.onServer(server -> {
			World.teleport(server, pose.get());
			this.rebuildWardrobe(server);
			World.equip(server);
		}));
		this.steps.add(this.chunksReady());
		this.steps.add(() -> this.hideHud(false));
		this.steps.add(this.waited(FADE));
		if (this.settings.includes(HAND_STILL)) {
			this.steps.add(this::clearToasts);
			this.steps.add(this.captured(HAND_STILL));
		}
		if (this.settings.includes(INVENTORY_STILL)) {
			this.steps.add(() -> {
				this.minecraft.gui.setScreen(new InventoryScreen(Objects.requireNonNull(this.minecraft.player)));
				return true;
			});
			this.steps.add(this.waited(SECOND));
			this.steps.add(this::clearToasts);
			this.steps.add(this.captured(INVENTORY_STILL));
			this.steps.add(() -> {
				this.minecraft.gui.setScreen(null);
				return true;
			});
		}
		this.steps.add(this.onServer(World::unequip));
		this.steps.add(() -> this.hideHud(true));
	}

	private void still(String name, Supplier<Pose> pose, Consumer<MinecraftServer> stage) {
		if (!this.settings.includes(name)) {
			return;
		}
		this.steps.add(this.onServer(server -> World.teleport(server, pose.get())));
		this.steps.add(this.chunksReady());
		this.steps.add(this.onServer(stage));
		this.steps.add(this.chunksReady());
		this.steps.add(this.waited(FADE));
		this.steps.add(this.captured(name));
	}

	private void fly(String name, Function<String, Flight> planned, Consumer<MinecraftServer> stage) {
		if (!this.settings.includes(name)) {
			return;
		}
		Supplier<Flight> flight = () -> planned.apply(name);
		this.steps.add(this.onServer(server -> World.teleport(server, flight.get().path().at(0))));
		this.steps.add(this.chunksReady());
		this.steps.add(this.onServer(stage));
		this.steps.add(this.chunksReady());
		this.steps.add(this.waited(FADE));
		this.steps.add(this.flown(flight, FLIGHT_WARMUP_TICKS));
	}

	private void arrive(String name, Supplier<Pose> origin, Supplier<Pose> destination) {
		if (!this.settings.includes(name)) {
			return;
		}
		this.steps.add(this.onServer(server -> World.teleport(server, destination.get())));
		this.steps.add(this.chunksReady());
		this.steps.add(this.onServer(server -> World.teleport(server, origin.get())));
		this.steps.add(this.onServer(World::clearEntities));
		this.steps.add(this.chunksReady());
		this.steps.add(this.flown(() -> Flight.hover(name, HOVER_TICKS, destination.get()), 0));
	}

	private boolean titleShown() {
		if (!this.minecraft.isGameLoadFinished() || this.minecraft.gui.overlay() != null || this.minecraft.gui.screen() == null) {
			return false;
		}
		long started = ManagementFactory.getRuntimeMXBean().getStartTime();
		this.report.addProperty("startup_seconds", (System.currentTimeMillis() - started) / 1000.0);
		return true;
	}

	private boolean openWorld() {
		boolean exists = this.minecraft.getLevelSource().levelExists(World.LEVEL);
		this.report.addProperty("generated", !exists);
		this.mark = System.nanoTime();
		if (exists) {
			this.minecraft.createWorldOpenFlows().openWorld(World.LEVEL, () -> {
				throw new IllegalStateException("opening the world was cancelled");
			});
		} else {
			LevelSettings level = new LevelSettings(
				World.LEVEL,
				GameType.SPECTATOR,
				new LevelSettings.DifficultySettings(Difficulty.PEACEFUL, false, false),
				true,
				WorldDataConfiguration.DEFAULT
			);
			this.minecraft
				.createWorldOpenFlows()
				.createFreshLevel(World.LEVEL, level, new WorldOptions(World.SEED, true, false), WorldPresets::createNormalWorldDimensions, this.minecraft.gui.screen());
		}
		return true;
	}

	private boolean joined() {
		if (this.minecraft.level == null || this.minecraft.player == null || this.minecraft.gui.screen() != null) {
			return false;
		}
		this.report.addProperty("join_seconds", (System.nanoTime() - this.mark) / (double) SECOND);
		return true;
	}

	private boolean clearToasts() {
		this.minecraft.gui.toastManager().clear();
		return true;
	}

	private boolean hideHud(boolean hidden) {
		if (this.minecraft.gui.hud.isHidden() != hidden) {
			this.minecraft.gui.hud.toggle();
		}
		return true;
	}

	private boolean finish() throws IOException {
		this.report.addProperty("width", this.minecraft.getWindow().getWidth());
		this.report.addProperty("height", this.minecraft.getWindow().getHeight());
		this.report.addProperty("heap_used_megabytes", ManagementFactory.getMemoryMXBean().getHeapMemoryUsage().getUsed() / (1024.0 * 1024.0));
		Files.createDirectories(this.settings.output());
		Files.writeString(this.settings.output().resolve(REPORT), new GsonBuilder().setPrettyPrinting().create().toJson(this.report));
		LOGGER.info("benchmark finished: {}", this.settings.output().resolve(REPORT));
		this.minecraft.stop();
		return true;
	}

	private Step onServer(Consumer<MinecraftServer> task) {
		return new Step() {
			private @Nullable CompletableFuture<Void> pending;

			@Override
			public boolean advance() {
				if (this.pending == null) {
					MinecraftServer server = Objects.requireNonNull(Runner.this.minecraft.getSingleplayerServer());
					this.pending = server.submit(() -> task.accept(server));
				}
				if (!this.pending.isDone()) {
					return false;
				}
				this.pending.join();
				return true;
			}
		};
	}

	private Step waited(long nanos) {
		return new Step() {
			private long until;

			@Override
			public boolean advance() {
				if (this.until == 0) {
					this.until = System.nanoTime() + nanos;
				}
				return System.nanoTime() >= this.until;
			}
		};
	}

	private Step chunksReady() {
		return new Step() {
			private long started;
			private long changed;
			private long busy;
			private int chunks = -1;
			private int uncompiled = -1;

			@Override
			public boolean advance() {
				long now = System.nanoTime();
				if (this.started == 0) {
					this.started = now;
					this.changed = now;
					this.busy = now;
				}
				int loaded = Objects.requireNonNull(Runner.this.minecraft.level).getChunkSource().getLoadedChunksCount();
				if (loaded != this.chunks) {
					this.chunks = loaded;
					this.changed = now;
				}
				int pending = 0;
				boolean resorting = false;
				for (SectionRenderDispatcher.RenderSection section : Runner.this.minecraft.levelRenderer.visibleSections()) {
					if (section.getSectionMesh() == CompiledSectionMesh.UNCOMPILED) {
						pending++;
					}
					resorting |= section.transparencyResortingScheduled();
				}
				if (pending != this.uncompiled || resorting || !Runner.this.minecraft.levelRenderer.hasRenderedAllSections()) {
					this.uncompiled = pending;
					this.busy = now;
				}
				if (now - this.started > CHUNKS_TIMEOUT) {
					throw new IllegalStateException("chunks did not settle");
				}
				return now - this.started >= CHUNKS_MINIMUM && now - this.changed >= CHUNKS_STABLE && now - this.busy >= CHUNKS_IDLE;
			}
		};
	}

	private Step captured(String name) {
		return new Step() {
			private @Nullable CompletableFuture<JsonObject> pending;

			@Override
			public boolean advance() throws IOException {
				if (this.pending == null) {
					Files.createDirectories(Runner.this.settings.output());
					this.pending = Runner.this.stills.capture(Runner.this.minecraft, name);
				}
				if (!this.pending.isDone()) {
					return false;
				}
				Runner.this.stillResults.add(name, this.pending.join());
				return true;
			}
		};
	}

	private Step flown(Supplier<Flight> planned, int warmupTicks) {
		return new Step() {
			private @Nullable Flight started;

			@Override
			public boolean advance() {
				if (this.started == null) {
					this.started = planned.get();
					Runner.this.flightTick = -warmupTicks;
					Runner.this.flight = this.started;
					return false;
				}
				if (Runner.this.flightTick < this.started.ticks()) {
					return false;
				}
				Runner.this.flight = null;
				Runner.this.flightResults.add(this.started.name(), Runner.this.frames.stop());
				LOGGER.info("flight {} finished", this.started.name());
				return true;
			}
		};
	}
}
