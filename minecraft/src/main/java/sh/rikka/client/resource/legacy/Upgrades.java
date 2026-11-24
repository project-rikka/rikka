package sh.rikka.client.resource.legacy;

import static sh.rikka.client.resource.legacy.PackFormats.V1_13;
import static sh.rikka.client.resource.legacy.PackFormats.V1_15;
import static sh.rikka.client.resource.legacy.PackFormats.V1_16_2;
import static sh.rikka.client.resource.legacy.PackFormats.V1_17;
import static sh.rikka.client.resource.legacy.PackFormats.V1_19;
import static sh.rikka.client.resource.legacy.PackFormats.V1_19_3;
import static sh.rikka.client.resource.legacy.PackFormats.V1_19_4;
import static sh.rikka.client.resource.legacy.PackFormats.V1_20;
import static sh.rikka.client.resource.legacy.PackFormats.V1_20_2;
import static sh.rikka.client.resource.legacy.PackFormats.V1_20_3;
import static sh.rikka.client.resource.legacy.PackFormats.V1_20_5;
import static sh.rikka.client.resource.legacy.PackFormats.V1_21_11;
import static sh.rikka.client.resource.legacy.PackFormats.V1_21_2;
import static sh.rikka.client.resource.legacy.PackFormats.V1_21_4;
import static sh.rikka.client.resource.legacy.PackFormats.V1_21_5;
import static sh.rikka.client.resource.legacy.PackFormats.V1_21_9;
import static sh.rikka.client.resource.legacy.PackFormats.V1_9;
import static sh.rikka.client.resource.legacy.PackFormats.V26_1;
import static sh.rikka.client.resource.legacy.PackFormats.V26_2;
import static sh.rikka.client.resource.legacy.PackFormats.V26_3;

import java.util.List;
import java.util.Optional;

final class Upgrades {
	private static final int ALWAYS = Integer.MAX_VALUE;
	private static final String BLOCK_MODELS = "block/";

	static final List<Step> STEPS = List.of(
		new Step(ALWAYS, new NameNormalization()),
		new Step(V1_9, new DialFrames()),
		new Step(V1_9, new RedstoneDust()),
		new Step(V1_9, new ItemDisplay()),
		new Step(V1_9, DataRenames.before(V1_9)),
		new Step(V1_13, conversion -> Renaming.rewriteBlockstateModels(conversion, id -> Optional.of(BLOCK_MODELS + id))),
		new Step(V1_13, DataRenames.before(V1_13)),
		new Step(V1_13, Upgrades::flattenDirectories),
		new Step(V1_13, new BlockstateVariants()),
		new Step(V1_13, new WaterTint()),
		new Step(V1_13, new LangFiles()),
		new Step(V1_13, conversion -> Renaming.moveTree(conversion, Assets.MINECRAFT + "mcpatcher/", Assets.MINECRAFT + "optifine/")),
		new Step(V1_15, new SheetSlicing(V1_15)),
		new Step(V1_15, new SweepFrames()),
		new Step(V1_15, DataRenames.before(V1_15)),
		new Step(V1_15, new ChestLayouts()),
		new Step(V1_16_2, DataRenames.before(V1_16_2)),
		new Step(V1_16_2, conversion -> Renaming.moveTree(conversion, Assets.MINECRAFT + "sounds/mob/zombiepig/", Assets.MINECRAFT + "sounds/mob/zombified_piglin/")),
		new Step(V1_17, DataRenames.before(V1_17)),
		new Step(V1_19, DataRenames.before(V1_19)),
		new Step(V1_19_3, new CreativeTabs()),
		new Step(V1_19_3, new CustomAtlases()),
		new Step(V1_19_3, DataRenames.before(V1_19_3)),
		new Step(V1_19_4, DataRenames.before(V1_19_4)),
		new Step(V1_20, new TitleLogo()),
		new Step(V1_20, DataRenames.before(V1_20)),
		new Step(V1_20_2, new ContainerLayouts()),
		new Step(V1_20_2, new SheetSlicing(V1_20_2)),
		new Step(V1_20_2, new LegacySprites()),
		new Step(V1_20_2, DataRenames.before(V1_20_2)),
		new Step(V1_20_3, DataRenames.before(V1_20_3)),
		new Step(V1_20_5, new SheetSlicing(V1_20_5)),
		new Step(V1_20_5, DataRenames.before(V1_20_5)),
		new Step(V1_21_2, DataRenames.before(V1_21_2)),
		new Step(V1_21_4, new SlotIcons()),
		new Step(V1_21_4, DataRenames.before(V1_21_4)),
		new Step(V1_21_4, new ItemDefinitions()),
		new Step(V1_21_5, new EntityCanvas()),
		new Step(V1_21_5, DataRenames.before(V1_21_5)),
		new Step(V1_21_5, conversion -> Renaming.moveFiles(conversion, Assets.MINECRAFT + "sounds/mob/wolf/", Assets.MINECRAFT + "sounds/mob/wolf/classic/")),
		new Step(V1_21_9, DataRenames.before(V1_21_9)),
		new Step(V1_21_11, new SheetSlicing(V1_21_11)),
		new Step(V1_21_11, DataRenames.before(V1_21_11)),
		new Step(V26_1, DataRenames.before(V26_1)),
		new Step(V26_2, new Beds()),
		new Step(V26_2, new Signs()),
		new Step(V26_2, DataRenames.before(V26_2)),
		new Step(V26_3, DataRenames.before(V26_3)),
		new Step(ALWAYS, new BlockstateValidation()),
		new Step(ALWAYS, new GuiSynthesis()),
		new Step(V1_20_2, new SpriteMetadata())
	);

	private Upgrades() {
	}

	private static void flattenDirectories(Conversion conversion) {
		Renaming.texturePrefix(conversion, "blocks/", "block/");
		Renaming.texturePrefix(conversion, "items/", "item/");
	}

	record Step(int before, Upgrade upgrade) {
	}
}
