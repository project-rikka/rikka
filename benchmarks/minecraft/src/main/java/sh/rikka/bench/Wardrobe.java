package sh.rikka.bench;

import java.util.List;
import java.util.Locale;
import net.minecraft.server.MinecraftServer;

final class Wardrobe {
	static final int COLUMNS = 9;
	static final int COLUMN_SPACING = 2;
	static final int FIRST_ROW = 4;
	static final int ROW_SPACING = 3;

	private static final String POSED = "NoAI:1b,NoGravity:1b,Silent:1b,Rotation:[90f,0f]";
	private static final String GLINT = "components:{\"minecraft:enchantment_glint_override\":true}";
	private static final List<List<String>> ENTITY_ROWS = List.of(
		List.of(
			"armor_stand %s {ShowArms:1b,NoGravity:1b,Rotation:[90f,0f],equipment:{"
				+ "head:{id:\"leather_helmet\",components:{\"minecraft:dyed_color\":11546150}},"
				+ "chest:{id:\"leather_chestplate\",components:{\"minecraft:dyed_color\":3949738}},"
				+ "legs:{id:\"leather_leggings\",components:{\"minecraft:dyed_color\":8439583}},"
				+ "feet:{id:\"leather_boots\",components:{\"minecraft:dyed_color\":16701501}},"
				+ "mainhand:{id:\"bow\"},offhand:{id:\"potion\",components:{\"minecraft:potion_contents\":{potion:\"minecraft:healing\"}}}}}",
			"armor_stand %s {ShowArms:1b,NoGravity:1b,Rotation:[90f,0f],equipment:{"
				+ "head:{id:\"netherite_helmet\",components:{\"minecraft:trim\":{material:\"minecraft:gold\",pattern:\"minecraft:coast\"},"
				+ "\"minecraft:enchantment_glint_override\":true}},"
				+ "chest:{id:\"netherite_chestplate\",components:{\"minecraft:trim\":{material:\"minecraft:redstone\",pattern:\"minecraft:silence\"}}},"
				+ "legs:{id:\"netherite_leggings\"},feet:{id:\"netherite_boots\"},"
				+ "mainhand:{id:\"trident\"},offhand:{id:\"shield\",components:{\"minecraft:base_color\":\"red\"}}}}",
			"zombie %s {" + POSED + ",equipment:{head:{id:\"iron_helmet\"," + GLINT + "},chest:{id:\"iron_chestplate\"," + GLINT + "},"
				+ "legs:{id:\"iron_leggings\"},feet:{id:\"iron_boots\"},mainhand:{id:\"diamond_sword\"," + GLINT + "}}}",
			"skeleton %s {" + POSED + ",equipment:{mainhand:{id:\"bow\"}}}",
			"spider %s {" + POSED + "}",
			"creeper %s {" + POSED + ",powered:1b}",
			"enderman %s {" + POSED + "}",
			"sheep %s {" + POSED + ",Color:14b}",
			"slime %s {" + POSED + ",Size:1}"
		),
		List.of(
			"horse %s {" + POSED + ",Variant:513,Tame:1b,equipment:{body:{id:\"diamond_horse_armor\"},saddle:{id:\"saddle\"}}}",
			"wolf %s {" + POSED + ",equipment:{body:{id:\"wolf_armor\"}}}",
			"villager %s {" + POSED + ",VillagerData:{profession:\"minecraft:librarian\",level:2,type:\"minecraft:plains\"}}",
			"iron_golem %s {" + POSED + "}",
			"shulker %s {" + POSED + ",Color:10b}",
			"armor_stand %s {ShowArms:1b,NoGravity:1b,Rotation:[90f,0f],equipment:{head:{id:\"player_head\"},chest:{id:\"elytra\"},"
				+ "mainhand:{id:\"enchanted_book\"},offhand:{id:\"spyglass\"}}}",
			"piglin %s {" + POSED + ",IsImmuneToZombification:1b,equipment:{mainhand:{id:\"golden_sword\"},offhand:{id:\"crossbow\"}}}",
			"pillager %s {" + POSED + ",equipment:{mainhand:{id:\"crossbow\"}}}",
			"witch %s {" + POSED + "}"
		),
		List.of(
			"oak_chest_boat %s {NoGravity:1b,Rotation:[90f,0f]}",
			"minecart %s {NoGravity:1b}",
			"item_display %s {Rotation:[90f,0f],item:{id:\"diamond_pickaxe\"," + GLINT + "}}",
			"item_display %s {Rotation:[90f,0f],item:{id:\"red_stained_glass\"}}",
			"item_display %s {Rotation:[90f,0f],item:{id:\"cow_spawn_egg\"}}",
			"allay %s {" + POSED + ",equipment:{mainhand:{id:\"amethyst_shard\"}}}",
			"fox %s {" + POSED + ",equipment:{mainhand:{id:\"sweet_berries\"}}}",
			"cat %s {" + POSED + "}",
			"phantom %s {" + POSED + "}"
		)
	);
	private static final List<String> BLOCKS = List.of(
		"red_banner[rotation=4]{patterns:[{pattern:\"minecraft:creeper\",color:\"black\"},{pattern:\"minecraft:border\",color:\"white\"}]}",
		"red_bed[facing=west,part=foot]",
		"oak_sign[rotation=4]{front_text:{messages:[\"Rikka\",\"Client\",\"\",\"\"]}}",
		"purple_shulker_box",
		"dragon_head[rotation=4]",
		"decorated_pot",
		"enchanting_table",
		"lectern[facing=west,has_book=true]",
		"ender_chest[facing=west]"
	);

	private Wardrobe() {
	}

	static double centerZ() {
		return World.CROWD_ORIGIN + World.CROWD_SIDE * World.CROWD_SPACING / 2.0;
	}

	static void build(MinecraftServer server, int floor) {
		for (int row = 0; row < ENTITY_ROWS.size(); row++) {
			List<String> entities = ENTITY_ROWS.get(row);
			for (int column = 0; column < entities.size(); column++) {
				String position = String.format(Locale.ROOT, "%.1f %d %.1f", rowX(row) + 0.5, floor + 1, columnZ(column) + 0.5);
				World.command(server, "summon " + String.format(Locale.ROOT, entities.get(column), position));
			}
		}
		for (int column = 0; column < BLOCKS.size(); column++) {
			int x = (int) rowX(ENTITY_ROWS.size());
			World.command(server, String.format(Locale.ROOT, "setblock %d %d %d %s", x, floor + 1, (int) columnZ(column), BLOCKS.get(column)));
		}
	}

	private static double rowX(int row) {
		return World.CROWD_ORIGIN + FIRST_ROW + row * ROW_SPACING;
	}

	private static double columnZ(int column) {
		return Math.floor(centerZ()) + (column - COLUMNS / 2) * COLUMN_SPACING;
	}
}
