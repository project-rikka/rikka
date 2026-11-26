package sh.rikka.client.render.culling;

import net.minecraft.client.Camera;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.blockentity.BlockEntityRenderDispatcher;
import net.minecraft.client.renderer.blockentity.BlockEntityRenderer;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
import net.minecraft.core.BlockPos;
import net.minecraft.world.entity.Avatar;
import net.minecraft.world.entity.Display;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.Leashable;
import net.minecraft.world.entity.item.PrimedTnt;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.mixin.culling.EntityRendererInvoker;
import sh.rikka.client.mods.boost.BoostFeature;
import sh.rikka.client.mods.visual.VisualHooks;

public final class OcclusionCulling {
	private static final double MAX_EXTENT = 16.0;
	private static final double BLOCK_ENTITY_REACH = 1.0;

	private static @Nullable CullingSession session;
	private static @Nullable CullingSession extracting;
	private static @Nullable AABB cullingBox;
	private static @Nullable CullTicket pendingBlockEntity;
	private static @Nullable Entity cameraRoot;
	private static @Nullable Entity playerRoot;
	private static @Nullable Entity target;
	private static boolean labelsPrimedTnt;
	private static @Nullable EntityRenderDispatcher entityRenderers;

	private OcclusionCulling() {
	}

	public static void beginExtraction(ClientLevel level, Camera camera, EntityRenderDispatcher dispatcher) {
		if (!BoostFeature.ENTITY_CULLING.isActive()) {
			close();
			return;
		}
		Vec3 position = camera.position();
		CullingSession current = session;
		if (current == null || current.level != level) {
			close();
			current = new CullingSession(level, position.x, position.y, position.z);
			session = current;
		}
		current.beginFrame(position.x, position.y, position.z);
		Minecraft minecraft = Minecraft.getInstance();
		Entity viewer = camera.entity();
		cameraRoot = viewer == null ? null : viewer.getRootVehicle();
		playerRoot = minecraft.player == null ? null : minecraft.player.getRootVehicle();
		target = RikkaClient.targets().target();
		labelsPrimedTnt = VisualHooks.labelsPrimedTnt();
		entityRenderers = dispatcher;
		extracting = current;
	}

	public static void endExtraction() {
		CullingSession current = extracting;
		if (current == null) {
			return;
		}
		extracting = null;
		cullingBox = null;
		pendingBlockEntity = null;
		cameraRoot = null;
		playerRoot = null;
		target = null;
		current.publish();
	}

	public static void levelChanged() {
		close();
	}

	public static void prepareEntity() {
		if (extracting != null) {
			cullingBox = null;
		}
	}

	public static void cullingBox(AABB box) {
		if (extracting != null) {
			cullingBox = box;
		}
	}

	public static boolean isEntityVisible(Entity entity) {
		CullingSession current = extracting;
		AABB box = cullingBox;
		if (current == null || box == null) {
			return true;
		}
		cullingBox = null;
		if (isExempt(current, entity, box)) {
			return true;
		}
		CullTicket ticket = ((Occludable) entity).rikka$cullTicket();
		if (current.isLeashed(ticket)) {
			return true;
		}
		boolean culled = current.refresh(ticket, box.minX, box.minY, box.minZ);
		current.record(ticket, box.minX, box.minY, box.minZ, box.maxX, box.maxY, box.maxZ);
		return !culled || showsNameTag(current, entity);
	}

	public static boolean hidesBlockEntity(BlockEntityRenderDispatcher dispatcher, BlockEntity blockEntity) {
		CullingSession current = extracting;
		pendingBlockEntity = null;
		if (current == null) {
			return false;
		}
		BlockEntityRenderer<?, ?> renderer = dispatcher.getRenderer(blockEntity);
		if (renderer == null || RendererTraits.extendsBlockEntityReach(renderer)) {
			return false;
		}
		CullTicket ticket = ((Occludable) blockEntity).rikka$cullTicket();
		BlockPos pos = blockEntity.getBlockPos();
		if (current.refresh(ticket, pos.getX() - BLOCK_ENTITY_REACH, pos.getY() - BLOCK_ENTITY_REACH, pos.getZ() - BLOCK_ENTITY_REACH)) {
			record(current, ticket, pos);
			return true;
		}
		pendingBlockEntity = ticket;
		return false;
	}

	public static void blockEntityDrawn(BlockEntity blockEntity) {
		CullingSession current = extracting;
		CullTicket ticket = pendingBlockEntity;
		if (current == null || ticket == null) {
			return;
		}
		pendingBlockEntity = null;
		record(current, ticket, blockEntity.getBlockPos());
	}

	public static void chunkLoaded(ClientLevel level, ChunkPos pos) {
		CullingSession current = sessionOf(level);
		if (current != null) {
			LevelChunk chunk = level.getChunkSource().getChunk(pos.x(), pos.z(), ChunkStatus.FULL, false);
			if (chunk != null) {
				current.chunkLoaded(chunk);
			}
		}
	}

	public static void chunkUnloaded(ClientLevel level, ChunkPos pos) {
		CullingSession current = sessionOf(level);
		if (current != null) {
			current.chunkUnloaded(pos.x(), pos.z());
		}
	}

	public static void blockChanged(ClientLevel level, BlockPos pos, BlockState previous, BlockState current) {
		CullingSession owner = sessionOf(level);
		if (owner == null) {
			return;
		}
		boolean opaque = current.isSolidRender();
		if (previous.isSolidRender() != opaque) {
			owner.blockChanged(pos.getX(), pos.getY(), pos.getZ(), opaque);
		}
	}

	private static @Nullable CullingSession sessionOf(ClientLevel level) {
		CullingSession current = session;
		return BoostFeature.ENTITY_CULLING.isActive() && current != null && current.level == level ? current : null;
	}

	private static void record(CullingSession current, CullTicket ticket, BlockPos pos) {
		current.record(
			ticket,
			pos.getX() - BLOCK_ENTITY_REACH,
			pos.getY() - BLOCK_ENTITY_REACH,
			pos.getZ() - BLOCK_ENTITY_REACH,
			pos.getX() + 1.0 + BLOCK_ENTITY_REACH,
			pos.getY() + 1.0 + BLOCK_ENTITY_REACH,
			pos.getZ() + 1.0 + BLOCK_ENTITY_REACH
		);
	}

	private static boolean isExempt(CullingSession current, Entity entity, AABB box) {
		if (Math.max(box.getXsize(), Math.max(box.getYsize(), box.getZsize())) > MAX_EXTENT || entity instanceof Avatar) {
			return true;
		}
		Entity root = entity.getRootVehicle();
		if (root == cameraRoot || root == playerRoot || entity == target || Minecraft.getInstance().shouldEntityAppearGlowing(entity)) {
			return true;
		}
		if (entity instanceof Leashable leashable) {
			Leashable.LeashData leash = leashable.getLeashData();
			if (leash != null) {
				if (leash.leashHolder != null) {
					current.markLeashed(((Occludable) leash.leashHolder).rikka$cullTicket());
				}
				return true;
			}
		}
		if (labelsPrimedTnt && entity instanceof PrimedTnt) {
			return true;
		}
		if (entity instanceof Display.TextDisplay text) {
			Display.TextDisplay.TextRenderState state = text.textRenderState();
			if (state == null || (state.flags() & Display.TextDisplay.FLAG_SEE_THROUGH) != 0) {
				return true;
			}
		}
		return RendererTraits.extendsEntityReach(renderers().getRenderer(entity));
	}

	private static boolean showsNameTag(CullingSession current, Entity entity) {
		double distance = entity.distanceToSqr(current.cameraX(), current.cameraY(), current.cameraZ());
		return ((EntityRendererInvoker) renderers().getRenderer(entity)).rikka$shouldShowName(entity, distance);
	}

	private static EntityRenderDispatcher renderers() {
		EntityRenderDispatcher dispatcher = entityRenderers;
		if (dispatcher == null) {
			throw new IllegalStateException("entity culling runs outside level extraction");
		}
		return dispatcher;
	}

	private static void close() {
		extracting = null;
		cullingBox = null;
		pendingBlockEntity = null;
		CullingSession current = session;
		if (current != null) {
			session = null;
			current.close();
		}
	}
}
