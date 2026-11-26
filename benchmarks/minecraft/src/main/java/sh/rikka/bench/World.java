package sh.rikka.bench;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.component.DataComponents;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.Difficulty;
import net.minecraft.world.clock.WorldClock;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.entity.decoration.ArmorStand;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.gamerules.GameRules;
import net.minecraft.world.level.levelgen.Heightmap;

final class World {
	static final long SEED = 20261125L;
	static final String LEVEL = "bench";
	static final int ORIGIN = 8;
	static final int CROWD_ORIGIN = 100;
	static final int CROWD_SIDE = 20;
	static final int CROWD_SPACING = 2;
	static final int CROWD_HEIGHT = 40;

	private static final int FLOOR_CLEARANCE = 2;
	private static final int NOON = 6000;
	private static final List<EntityType<? extends Entity>> CROWD = List.of(
		EntityTypes.COW,
		EntityTypes.SHEEP,
		EntityTypes.PIG,
		EntityTypes.CHICKEN,
		EntityTypes.VILLAGER,
		EntityTypes.HORSE,
		EntityTypes.IRON_GOLEM,
		EntityTypes.WOLF
	);
	private static final Map<EquipmentSlot, Item> GEAR = Map.of(
		EquipmentSlot.HEAD, Items.DIAMOND_HELMET,
		EquipmentSlot.CHEST, Items.DIAMOND_CHESTPLATE,
		EquipmentSlot.LEGS, Items.DIAMOND_LEGGINGS,
		EquipmentSlot.FEET, Items.DIAMOND_BOOTS,
		EquipmentSlot.MAINHAND, Items.DIAMOND_SWORD,
		EquipmentSlot.OFFHAND, Items.SHIELD
	);
	private static final List<String> EQUIPMENT = List.of(
		"gamemode survival @a",
		"item replace entity @a weapon.mainhand with diamond_sword[enchantment_glint_override=true]",
		"item replace entity @a weapon.offhand with shield",
		"item replace entity @a hotbar.1 with potion[potion_contents={potion:\"minecraft:healing\"}]",
		"item replace entity @a hotbar.2 with red_stained_glass",
		"item replace entity @a hotbar.3 with player_head",
		"item replace entity @a hotbar.4 with bow",
		"item replace entity @a hotbar.5 with chest",
		"item replace entity @a armor.head with leather_helmet[dyed_color=11546150]",
		"item replace entity @a armor.chest with chainmail_chestplate[enchantment_glint_override=true]",
		"item replace entity @a armor.legs with netherite_leggings[trim={material:\"minecraft:gold\",pattern:\"minecraft:coast\"}]",
		"item replace entity @a armor.feet with golden_boots"
	);

	private World() {
	}

	static int prepare(MinecraftServer server) {
		ServerLevel level = server.overworld();
		GameRules rules = level.getGameRules();
		rules.set(GameRules.ADVANCE_TIME, false, server);
		rules.set(GameRules.ADVANCE_WEATHER, false, server);
		rules.set(GameRules.SPAWN_MOBS, false, server);
		rules.set(GameRules.SPAWN_MONSTERS, false, server);
		rules.set(GameRules.SPAWN_PATROLS, false, server);
		rules.set(GameRules.SPAWN_PHANTOMS, false, server);
		rules.set(GameRules.SPAWN_WANDERING_TRADERS, false, server);
		rules.set(GameRules.SHOW_ADVANCEMENT_MESSAGES, false, server);
		rules.set(GameRules.PLAYER_MOVEMENT_CHECK, false, server);
		rules.set(GameRules.RANDOM_TICK_SPEED, 0, server);
		server.setDifficulty(Difficulty.PEACEFUL, true);
		level.resetWeatherCycle();
		Holder<WorldClock> clock = level.dimensionType().defaultClock().orElseThrow();
		server.clockManager().setTotalTicks(clock, NOON);
		for (ServerPlayer player : server.getPlayerList().getPlayers()) {
			player.setGameMode(GameType.SPECTATOR);
		}
		level.getChunk(ORIGIN >> 4, ORIGIN >> 4);
		return level.getHeight(Heightmap.Types.MOTION_BLOCKING, ORIGIN, ORIGIN);
	}

	static void freeze(MinecraftServer server, boolean frozen) {
		server.tickRateManager().setFrozen(frozen);
	}

	static void teleport(MinecraftServer server, Pose pose) {
		for (ServerPlayer player : server.getPlayerList().getPlayers()) {
			player.teleportTo(server.overworld(), pose.x(), pose.y(), pose.z(), Set.of(), pose.yaw(), pose.pitch(), false);
		}
	}

	static void clearEntities(MinecraftServer server) {
		List<Entity> entities = new ArrayList<>();
		for (Entity entity : server.overworld().getAllEntities()) {
			if (!(entity instanceof Player)) {
				entities.add(entity);
			}
		}
		entities.forEach(Entity::discard);
	}

	static void buildGear(MinecraftServer server, int floor) {
		ServerLevel level = server.overworld();
		buildFloor(level, floor);
		for (int row = 1; row < CROWD_SIDE; row++) {
			for (int column = 0; column < CROWD_SIDE; column++) {
				ArmorStand stand = EntityTypes.ARMOR_STAND.create(level, EntitySpawnReason.COMMAND);
				if (stand == null) {
					throw new IllegalStateException("failed to create an armor stand");
				}
				stand.snapTo(CROWD_ORIGIN + column * CROWD_SPACING + 0.5, floor + 1, CROWD_ORIGIN + row * CROWD_SPACING + 0.5, 90.0F, 0.0F);
				stand.setNoGravity(true);
				stand.setShowArms(true);
				boolean enchanted = (row + column) % 2 == 0;
				for (Map.Entry<EquipmentSlot, Item> gear : GEAR.entrySet()) {
					ItemStack stack = new ItemStack(gear.getValue());
					if (enchanted) {
						stack.set(DataComponents.ENCHANTMENT_GLINT_OVERRIDE, true);
					}
					stand.setItemSlot(gear.getKey(), stack);
				}
				level.addFreshEntity(stand);
			}
		}
	}

	private static void buildFloor(ServerLevel level, int floor) {
		int extent = CROWD_SIDE * CROWD_SPACING;
		for (int x = 0; x < extent; x++) {
			for (int z = 0; z < extent; z++) {
				level.setBlockAndUpdate(new BlockPos(CROWD_ORIGIN + x, floor, CROWD_ORIGIN + z), Blocks.STONE.defaultBlockState());
				for (int y = 1; y <= FLOOR_CLEARANCE; y++) {
					level.setBlockAndUpdate(new BlockPos(CROWD_ORIGIN + x, floor + y, CROWD_ORIGIN + z), Blocks.AIR.defaultBlockState());
				}
			}
			level.setBlockAndUpdate(new BlockPos(CROWD_ORIGIN + x, floor + 1, CROWD_ORIGIN), Blocks.CHEST.defaultBlockState());
		}
	}

	static void buildWardrobe(MinecraftServer server, int floor) {
		server.setDifficulty(Difficulty.EASY, true);
		buildFloor(server.overworld(), floor);
		Wardrobe.build(server, floor);
	}

	static void pacify(MinecraftServer server) {
		server.setDifficulty(Difficulty.PEACEFUL, true);
	}

	static void equip(MinecraftServer server) {
		for (String command : EQUIPMENT) {
			command(server, command);
		}
	}

	static void unequip(MinecraftServer server) {
		command(server, "clear @a");
		command(server, "gamemode spectator @a");
	}

	static void command(MinecraftServer server, String command) {
		CommandSourceStack source = server.createCommandSourceStack().withSuppressedOutput().withCallback((success, result) -> {
			if (!success) {
				throw new IllegalStateException("command failed: " + command);
			}
		});
		server.getCommands().performPrefixedCommand(source, command);
	}

	static void buildCrowd(MinecraftServer server, int floor) {
		ServerLevel level = server.overworld();
		buildFloor(level, floor);
		for (int row = 1; row < CROWD_SIDE; row++) {
			for (int column = 0; column < CROWD_SIDE; column++) {
				Entity entity = CROWD.get((row + column) % CROWD.size()).create(level, EntitySpawnReason.COMMAND);
				if (entity == null) {
					throw new IllegalStateException("failed to create a crowd entity");
				}
				entity.snapTo(CROWD_ORIGIN + column * CROWD_SPACING + 0.5, floor + 1, CROWD_ORIGIN + row * CROWD_SPACING + 0.5, 90.0F, 0.0F);
				entity.setNoGravity(true);
				entity.setSilent(true);
				if (entity instanceof Mob mob) {
					mob.setNoAi(true);
					mob.setPersistenceRequired();
					mob.setYHeadRot(90.0F);
					mob.setYBodyRot(90.0F);
				}
				level.addFreshEntity(entity);
			}
		}
	}
}
