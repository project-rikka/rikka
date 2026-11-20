#include "engine.h"

#include "box3d/box3d.h"
#include "box3d/collision.h"

#include <stdlib.h>
#include <string.h>

typedef struct box3d_geometry {
	uint64_t key;
	b3HullData* hull;
	b3MeshData* mesh;
} box3d_geometry;

struct bench_world {
	b3WorldId id;
	uint32_t substeps;
	float sleep_threshold;
	b3BodyId* bodies;
	uint32_t body_count;
	uint32_t body_capacity;
	box3d_geometry* geometries;
	uint32_t geometry_count;
	uint32_t geometry_capacity;
};

static b3Vec3 box3d_vec( bench_vec3 v )
{
	return ( b3Vec3 ){ v.x, v.y, v.z };
}

static b3Quat box3d_quat( bench_quat q )
{
	return ( b3Quat ){ { q.x, q.y, q.z }, q.w };
}

static b3Transform box3d_transform( bench_pose pose )
{
	return ( b3Transform ){ box3d_vec( pose.p ), box3d_quat( pose.q ) };
}

static box3d_geometry* box3d_find( bench_world* world, uint64_t key )
{
	for ( uint32_t i = 0; i < world->geometry_count; ++i )
	{
		if ( world->geometries[i].key == key )
		{
			return world->geometries + i;
		}
	}
	if ( world->geometry_count == world->geometry_capacity )
	{
		uint32_t capacity = world->geometry_capacity == 0 ? 16 : 2 * world->geometry_capacity;
		box3d_geometry* grown = realloc( world->geometries, capacity * sizeof( box3d_geometry ) );
		if ( grown == NULL )
		{
			return NULL;
		}
		world->geometries = grown;
		world->geometry_capacity = capacity;
	}
	box3d_geometry* entry = world->geometries + world->geometry_count++;
	entry->key = key;
	entry->hull = NULL;
	entry->mesh = NULL;
	return entry;
}

static bool box3d_shape( bench_world* world, b3BodyId body, const bench_shape* shape )
{
	b3ShapeDef def = b3DefaultShapeDef();
	def.density = shape->density;
	def.baseMaterial.friction = shape->friction;
	def.baseMaterial.restitution = shape->restitution;
	def.baseMaterial.rollingResistance = shape->rolling_resistance;
	def.filter.categoryBits = shape->category;
	def.filter.maskBits = shape->mask;

	switch ( shape->geometry )
	{
		case BENCH_CUBOID:
		{
			bench_vec3 h = shape->half_extents;
			b3BoxHull box = b3MakeTransformedBoxHull( h.x, h.y, h.z, box3d_transform( shape->pose ) );
			return B3_IS_NON_NULL( b3CreateHullShape( body, &def, &box.base ) );
		}
		case BENCH_SPHERE:
		{
			b3Sphere sphere = { box3d_vec( shape->pose.p ), shape->radius };
			return B3_IS_NON_NULL( b3CreateSphereShape( body, &def, &sphere ) );
		}
		case BENCH_HULL:
		{
			box3d_geometry* entry = box3d_find( world, shape->key );
			if ( entry == NULL )
			{
				return false;
			}
			if ( entry->hull == NULL )
			{
				int count = (int)shape->point_count;
				entry->hull = b3CreateHull( (const b3Vec3*)shape->points, count, count );
				if ( entry->hull == NULL )
				{
					return false;
				}
			}
			return B3_IS_NON_NULL( b3CreateHullShape( body, &def, entry->hull ) );
		}
		case BENCH_MESH:
		{
			box3d_geometry* entry = box3d_find( world, shape->key );
			if ( entry == NULL )
			{
				return false;
			}
			if ( entry->mesh == NULL )
			{
				b3MeshDef mesh = { 0 };
				mesh.vertices = (b3Vec3*)shape->points;
				mesh.stride = sizeof( b3Vec3 );
				mesh.indices = (int32_t*)shape->indices;
				mesh.vertexCount = (int)shape->point_count;
				mesh.triangleCount = (int)( shape->index_count / 3 );
				mesh.identifyEdges = true;
				entry->mesh = b3CreateMesh( &mesh, NULL, 0 );
				if ( entry->mesh == NULL )
				{
					return false;
				}
			}
			return B3_IS_NON_NULL( b3CreateMeshShape( body, &def, entry->mesh, ( b3Vec3 ){ 1.0f, 1.0f, 1.0f } ) );
		}
		default:
			return false;
	}
}

static bench_world* box3d_create( const bench_world_def* def )
{
	bench_world* world = calloc( 1, sizeof( bench_world ) );
	if ( world == NULL )
	{
		return NULL;
	}
	b3WorldDef worldDef = b3DefaultWorldDef();
	worldDef.gravity = box3d_vec( def->gravity );
	worldDef.workerCount = def->workers;
	worldDef.enableSleep = def->sleeping;
	worldDef.enableContinuous = false;
	world->id = b3CreateWorld( &worldDef );
	world->substeps = def->substeps;
	world->sleep_threshold = def->sleep_threshold;
	world->body_capacity = def->max_bodies > 0 ? def->max_bodies : 16;
	world->bodies = malloc( world->body_capacity * sizeof( b3BodyId ) );
	if ( world->bodies == NULL )
	{
		b3DestroyWorld( world->id );
		free( world );
		return NULL;
	}
	return world;
}

static void box3d_destroy( bench_world* world )
{
	b3DestroyWorld( world->id );
	for ( uint32_t i = 0; i < world->geometry_count; ++i )
	{
		if ( world->geometries[i].hull != NULL )
		{
			b3DestroyHull( world->geometries[i].hull );
		}
		if ( world->geometries[i].mesh != NULL )
		{
			b3DestroyMesh( world->geometries[i].mesh );
		}
	}
	free( world->geometries );
	free( world->bodies );
	free( world );
}

static uint32_t box3d_add_body( bench_world* world, const bench_body* body )
{
	if ( world->body_count == world->body_capacity )
	{
		uint32_t capacity = 2 * world->body_capacity;
		b3BodyId* grown = realloc( world->bodies, capacity * sizeof( b3BodyId ) );
		if ( grown == NULL )
		{
			return BENCH_INVALID_BODY;
		}
		world->bodies = grown;
		world->body_capacity = capacity;
	}
	b3BodyDef def = b3DefaultBodyDef();
	def.type = (b3BodyType)body->motion;
	def.position = box3d_vec( body->pose.p );
	def.rotation = box3d_quat( body->pose.q );
	def.linearVelocity = box3d_vec( body->linear_velocity );
	def.angularVelocity = box3d_vec( body->angular_velocity );
	def.enableSleep = body->sleep;
	def.sleepThreshold = world->sleep_threshold;
	b3BodyId id = b3CreateBody( world->id, &def );
	for ( uint32_t i = 0; i < body->shape_count; ++i )
	{
		if ( !box3d_shape( world, id, body->shapes + i ) )
		{
			b3DestroyBody( id );
			return BENCH_INVALID_BODY;
		}
	}
	world->bodies[world->body_count] = id;
	return world->body_count++;
}

static bool box3d_add_joint( bench_world* world, const bench_joint* joint )
{
	if ( joint->body_a >= world->body_count || joint->body_b >= world->body_count )
	{
		return false;
	}
	b3BodyId a = world->bodies[joint->body_a];
	b3BodyId b = world->bodies[joint->body_b];
	b3Transform frameA = box3d_transform( joint->frame_a );
	b3Transform frameB = box3d_transform( joint->frame_b );
	switch ( joint->kind )
	{
		case BENCH_SPHERICAL:
		{
			b3SphericalJointDef def = b3DefaultSphericalJointDef();
			def.base.bodyIdA = a;
			def.base.bodyIdB = b;
			def.base.localFrameA = frameA;
			def.base.localFrameB = frameB;
			return B3_IS_NON_NULL( b3CreateSphericalJoint( world->id, &def ) );
		}
		case BENCH_REVOLUTE:
		{
			b3RevoluteJointDef def = b3DefaultRevoluteJointDef();
			def.base.bodyIdA = a;
			def.base.bodyIdB = b;
			def.base.localFrameA = frameA;
			def.base.localFrameB = frameB;
			def.enableMotor = joint->motor;
			def.motorSpeed = joint->motor_speed;
			def.maxMotorTorque = joint->max_motor_torque;
			return B3_IS_NON_NULL( b3CreateRevoluteJoint( world->id, &def ) );
		}
		default:
			return false;
	}
}

static void box3d_prepare( bench_world* world )
{
	(void)world;
}

static void box3d_move_kinematic( bench_world* world, uint32_t body, bench_pose target, float time_step )
{
	b3Body_SetTargetTransform( world->bodies[body], box3d_transform( target ), time_step, true );
}

static void box3d_step( bench_world* world, float time_step )
{
	b3World_Step( world->id, time_step, (int)world->substeps );
}

static bench_pose box3d_pose( const bench_world* world, uint32_t body )
{
	b3WorldTransform t = b3Body_GetTransform( world->bodies[body] );
	return ( bench_pose ){ { t.p.x, t.p.y, t.p.z }, { t.q.v.x, t.q.v.y, t.q.v.z, t.q.s } };
}

static uint32_t box3d_awake_bodies( const bench_world* world )
{
	return (uint32_t)b3World_GetAwakeBodyCount( world->id );
}

static const bench_engine box3d_engine = {
	box3d_create, box3d_destroy, box3d_add_body, box3d_add_joint, box3d_prepare,
	box3d_move_kinematic, box3d_step, box3d_pose, box3d_awake_bodies,
};

const bench_engine* box3d_bench_engine( void )
{
	return &box3d_engine;
}
