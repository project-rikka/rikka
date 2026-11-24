package sh.rikka.client.mods.animation;

import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.TickEvent;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.RegistrySetSetting;

public final class OldAnimationsMod extends Mod {
	private static final double MAX_SNEAK_DEPTH = 0.35;
	private static final double LEGACY_SNEAK_DEPTH = 0.08;
	private static final double SNEAK_DEPTH_STEP = 0.01;
	private static volatile @Nullable OldAnimationsMod active;

	private final BoolSetting swordBlocking = bool("sword_blocking", true);
	private final RegistrySetSetting<Item> blockingItems = registrySet("blocking_items", BuiltInRegistries.ITEM, swords())
		.visibleWhen(this.swordBlocking::get);
	private final BoolSetting blockHit = bool("block_hit", true);
	private final EnumSetting<ThirdPersonBlock> thirdPersonBlock = choice("third_person_block", ThirdPersonBlock.V1_7);
	private final BoolSetting firstPersonItems = bool("first_person_items", true);
	private final BoolSetting thirdPersonItems = bool("third_person_items", true);
	private final BoolSetting swingWhileUsing = bool("swing_while_using", true);
	private final BoolSetting punchBlocks = bool("punch_blocks", true);
	private final BoolSetting sneakCamera = bool("sneak_camera", true);
	private final DoubleSetting sneakDepth = number("sneak_depth", LEGACY_SNEAK_DEPTH, 0.0, MAX_SNEAK_DEPTH)
		.step(SNEAK_DEPTH_STEP)
		.visibleWhen(this.sneakCamera::get);
	private final BoolSetting sneakEase = bool("sneak_ease", true).visibleWhen(this.sneakCamera::get);
	private final BoolSetting sneakPose = bool("sneak_pose", true);
	private final BoolSetting heartFlash = bool("heart_flash", false);
	private final BoolSetting armorHitTint = bool("armor_hit_tint", true);
	private final SwordBlocking blocking = new SwordBlocking(this);

	public OldAnimationsMod() {
		super("old_animations", Category.VISUAL, "swords");
	}

	public static @Nullable OldAnimationsMod active() {
		return active;
	}

	private static List<Item> swords() {
		return BuiltInRegistries.ITEM.stream().filter(OldAnimationsMod::isSword).toList();
	}

	private static boolean isSword(Item item) {
		Identifier key = BuiltInRegistries.ITEM.getKey(item);
		return key.getNamespace().equals(Identifier.DEFAULT_NAMESPACE) && key.getPath().endsWith("_sword");
	}

	@Override
	protected void onEnable() {
		this.blocking.reset();
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
		this.blocking.reset();
	}

	@EventHandler
	private void onTick(TickEvent.Post event) {
		Minecraft minecraft = Minecraft.getInstance();
		this.blocking.tick(minecraft);
		if (this.punchBlocks.get()) {
			BlockPunch.tick(minecraft);
		}
	}

	boolean blocksWith(ItemStack stack) {
		return this.swordBlocking.get() && !stack.isEmpty() && this.blockingItems.contains(stack.getItem());
	}

	SwordBlocking blocking() {
		return this.blocking;
	}

	boolean blockHit() {
		return this.blockHit.get();
	}

	ThirdPersonBlock thirdPersonBlock() {
		return this.thirdPersonBlock.get();
	}

	boolean firstPersonItems() {
		return this.firstPersonItems.get();
	}

	boolean thirdPersonItems() {
		return this.thirdPersonItems.get();
	}

	boolean swingWhileUsing() {
		return this.swingWhileUsing.get();
	}

	boolean sneakCamera() {
		return this.sneakCamera.get();
	}

	float sneakDepth() {
		return this.sneakDepth.getFloat();
	}

	boolean sneakEase() {
		return this.sneakEase.get();
	}

	boolean sneakPose() {
		return this.sneakPose.get();
	}

	boolean heartFlash() {
		return this.heartFlash.get();
	}

	boolean armorHitTint() {
		return this.armorHitTint.get();
	}
}
