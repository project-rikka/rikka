#include "engine.h"

#include <Jolt/Jolt.h>

#include <Jolt/Core/Factory.h>
#include <Jolt/Core/JobSystemThreadPool.h>
#include <Jolt/Core/TempAllocator.h>
#include <Jolt/Physics/Body/BodyCreationSettings.h>
#include <Jolt/Physics/Collision/BroadPhase/BroadPhaseLayerInterfaceMask.h>
#include <Jolt/Physics/Collision/BroadPhase/ObjectVsBroadPhaseLayerFilterMask.h>
#include <Jolt/Physics/Collision/ObjectLayerPairFilterMask.h>
#include <Jolt/Physics/Collision/Shape/BoxShape.h>
#include <Jolt/Physics/Collision/Shape/ConvexHullShape.h>
#include <Jolt/Physics/Collision/Shape/MeshShape.h>
#include <Jolt/Physics/Collision/Shape/RotatedTranslatedShape.h>
#include <Jolt/Physics/Collision/Shape/SphereShape.h>
#include <Jolt/Physics/Collision/Shape/StaticCompoundShape.h>
#include <Jolt/Physics/Constraints/HingeConstraint.h>
#include <Jolt/Physics/Constraints/PointConstraint.h>
#include <Jolt/Physics/PhysicsSettings.h>
#include <Jolt/Physics/PhysicsSystem.h>
#include <Jolt/RegisterTypes.h>

#include <algorithm>
#include <cfloat>
#include <cstdio>
#include <cstdlib>
#include <map>
#include <memory>
#include <mutex>
#include <new>
#include <utility>
#include <vector>

namespace
{

constexpr JPH::uint32 kStaticGroup = 0x8000;
constexpr JPH::uint32 kGroupBits = 0x7fff;
constexpr JPH::uint32 kTempBytes = 64 * 1024 * 1024;
constexpr JPH::uint kMinPairs = 64 * 1024;
constexpr JPH::uint kMaxPairs = 256 * 1024;

std::once_flag gRegistered;

void Register()
{
	std::call_once( gRegistered, [] {
		JPH::RegisterDefaultAllocator();
		JPH::Factory::sInstance = new JPH::Factory();
		JPH::RegisterTypes();
	} );
}

JPH::Vec3 ToVec( bench_vec3 v )
{
	return JPH::Vec3( v.x, v.y, v.z );
}

JPH::Quat ToQuat( bench_quat q )
{
	return JPH::Quat( q.x, q.y, q.z, q.w ).Normalized();
}

bench_vec3 FromVec( JPH::Vec3 v )
{
	return bench_vec3{ v.GetX(), v.GetY(), v.GetZ() };
}

bool IsIdentity( bench_pose pose )
{
	return pose.p.x == 0.0f && pose.p.y == 0.0f && pose.p.z == 0.0f && pose.q.x == 0.0f && pose.q.y == 0.0f &&
		   pose.q.z == 0.0f && pose.q.w == 1.0f;
}

JPH::ObjectLayer LayerOf( uint32_t motion, uint32_t category, uint32_t mask )
{
	JPH::uint32 group = category & kGroupBits;
	if ( motion == BENCH_STATIC )
	{
		group |= kStaticGroup;
	}
	return JPH::ObjectLayerPairFilterMask::sGetObjectLayer( group, mask & ( kGroupBits | kStaticGroup ) );
}

struct JoltWorld
{
	explicit JoltWorld( const bench_world_def& def )
		: layers( 2 )
		, objectVsBroadPhase( layers )
		, temp( kTempBytes )
		, jobs( JPH::cMaxPhysicsJobs, JPH::cMaxPhysicsBarriers, std::max<int>( 0, int( def.workers ) - 1 ) )
	{
		layers.ConfigureLayer( JPH::BroadPhaseLayer( 0 ), kStaticGroup, 0 );
		layers.ConfigureLayer( JPH::BroadPhaseLayer( 1 ), kGroupBits, kStaticGroup );
		JPH::uint maxBodies = std::max<JPH::uint>( def.max_bodies, 1024 );
		JPH::uint pairs = std::clamp<JPH::uint>( 4 * maxBodies, kMinPairs, kMaxPairs );
		system.Init( maxBodies, 0, pairs, pairs, layers, objectVsBroadPhase, pairFilter );
		system.SetGravity( ToVec( def.gravity ) );
		JPH::PhysicsSettings settings;
		settings.mAllowSleeping = def.sleeping;
		settings.mPointVelocitySleepThreshold = def.sleep_threshold;
		system.SetPhysicsSettings( settings );
		sleeping = def.sleeping;
		bodies.reserve( def.max_bodies );
	}

	JPH::BroadPhaseLayerInterfaceMask layers;
	JPH::ObjectVsBroadPhaseLayerFilterMask objectVsBroadPhase;
	JPH::ObjectLayerPairFilterMask pairFilter;
	JPH::TempAllocatorImplWithMallocFallback temp;
	JPH::JobSystemThreadPool jobs;
	JPH::PhysicsSystem system;
	std::vector<JPH::BodyID> bodies;
	std::vector<JPH::BodyID> pending;
	std::map<std::pair<uint64_t, float>, JPH::ShapeRefC> shapes;
	bool sleeping = true;
	bool prepared = false;

	JPH::BodyInterface& Bodies()
	{
		return system.GetBodyInterfaceNoLock();
	}

	const JPH::BodyInterface& Bodies() const
	{
		return system.GetBodyInterfaceNoLock();
	}

	JPH::ShapeRefC Geometry( const bench_shape& shape )
	{
		auto key = std::make_pair( shape.key, shape.density );
		auto found = shapes.find( key );
		if ( found != shapes.end() )
		{
			return found->second;
		}
		JPH::ShapeRefC created = MakeGeometry( shape );
		if ( created != nullptr )
		{
			shapes.emplace( key, created );
		}
		return created;
	}

	JPH::ShapeRefC MakeGeometry( const bench_shape& shape ) const
	{
		switch ( shape.geometry )
		{
			case BENCH_CUBOID:
			{
				JPH::Vec3 half = ToVec( shape.half_extents );
				float radius = std::min( JPH::cDefaultConvexRadius, 0.5f * half.ReduceMin() );
				JPH::BoxShapeSettings settings( half, radius );
				settings.mDensity = shape.density;
				return Created( settings );
			}
			case BENCH_SPHERE:
			{
				JPH::SphereShapeSettings settings( shape.radius );
				settings.mDensity = shape.density;
				return Created( settings );
			}
			case BENCH_HULL:
			{
				JPH::Array<JPH::Vec3> points;
				points.reserve( shape.point_count );
				for ( uint32_t i = 0; i < shape.point_count; ++i )
				{
					points.push_back( ToVec( shape.points[i] ) );
				}
				JPH::ConvexHullShapeSettings settings( points );
				settings.mDensity = shape.density;
				return Created( settings );
			}
			case BENCH_MESH:
			{
				JPH::VertexList vertices;
				vertices.reserve( shape.point_count );
				for ( uint32_t i = 0; i < shape.point_count; ++i )
				{
					vertices.push_back( JPH::Float3( shape.points[i].x, shape.points[i].y, shape.points[i].z ) );
				}
				JPH::IndexedTriangleList triangles;
				triangles.reserve( shape.index_count / 3 );
				for ( uint32_t i = 0; i + 2 < shape.index_count; i += 3 )
				{
					triangles.push_back(
						JPH::IndexedTriangle( shape.indices[i], shape.indices[i + 1], shape.indices[i + 2] ) );
				}
				JPH::MeshShapeSettings settings( std::move( vertices ), std::move( triangles ) );
				return Created( settings );
			}
			default:
				return nullptr;
		}
	}

	static JPH::ShapeRefC Created( const JPH::ShapeSettings& settings )
	{
		JPH::ShapeSettings::ShapeResult result = settings.Create();
		return result.HasError() ? nullptr : result.Get();
	}

	JPH::ShapeRefC Placed( const bench_shape& shape )
	{
		JPH::ShapeRefC geometry = Geometry( shape );
		if ( geometry == nullptr || IsIdentity( shape.pose ) )
		{
			return geometry;
		}
		return new JPH::RotatedTranslatedShape( ToVec( shape.pose.p ), ToQuat( shape.pose.q ), geometry );
	}

	JPH::ShapeRefC Compose( const bench_body& body )
	{
		if ( body.shape_count == 1 )
		{
			return Placed( body.shapes[0] );
		}
		JPH::StaticCompoundShapeSettings compound;
		for ( uint32_t i = 0; i < body.shape_count; ++i )
		{
			const bench_shape& shape = body.shapes[i];
			JPH::ShapeRefC geometry = Geometry( shape );
			if ( geometry == nullptr )
			{
				return nullptr;
			}
			compound.AddShape( ToVec( shape.pose.p ), ToQuat( shape.pose.q ), geometry );
		}
		return Created( compound );
	}

	JPH::RMat44 FrameInWorld( uint32_t body, bench_pose frame ) const
	{
		return Bodies().GetWorldTransform( bodies[body] ) *
			   JPH::Mat44::sRotationTranslation( ToQuat( frame.q ), ToVec( frame.p ) );
	}
};

bench_world* Create( const bench_world_def* def )
{
	Register();
	return reinterpret_cast<bench_world*>( new ( std::nothrow ) JoltWorld( *def ) );
}

void Destroy( bench_world* handle )
{
	JoltWorld* world = reinterpret_cast<JoltWorld*>( handle );
	JPH::BodyInterface& bodies = world->Bodies();
	if ( !world->pending.empty() )
	{
		bodies.DestroyBodies( world->pending.data(), int( world->pending.size() ) );
	}
	delete world;
}

uint32_t AddBody( bench_world* handle, const bench_body* body )
{
	JoltWorld* world = reinterpret_cast<JoltWorld*>( handle );
	if ( body->shape_count == 0 )
	{
		return BENCH_INVALID_BODY;
	}
	JPH::ShapeRefC shape = world->Compose( *body );
	if ( shape == nullptr )
	{
		return BENCH_INVALID_BODY;
	}
	JPH::EMotionType motion = body->motion == BENCH_DYNAMIC	  ? JPH::EMotionType::Dynamic
							  : body->motion == BENCH_KINEMATIC ? JPH::EMotionType::Kinematic
																: JPH::EMotionType::Static;
	const bench_shape& first = body->shapes[0];
	JPH::BodyCreationSettings settings( shape, JPH::RVec3( ToVec( body->pose.p ) ), ToQuat( body->pose.q ), motion,
										LayerOf( body->motion, first.category, first.mask ) );
	settings.mFriction = first.friction;
	settings.mRestitution = first.restitution;
	settings.mAllowSleeping = world->sleeping && body->sleep;
	settings.mLinearVelocity = ToVec( body->linear_velocity );
	settings.mAngularVelocity = ToVec( body->angular_velocity );
	JPH::BodyInterface& bodies = world->Bodies();
	JPH::Body* created = bodies.CreateBody( settings );
	if ( created == nullptr )
	{
		return BENCH_INVALID_BODY;
	}
	JPH::BodyID id = created->GetID();
	if ( world->prepared )
	{
		JPH::EActivation activation =
			body->motion == BENCH_STATIC ? JPH::EActivation::DontActivate : JPH::EActivation::Activate;
		bodies.AddBody( id, activation );
	}
	else
	{
		world->pending.push_back( id );
	}
	world->bodies.push_back( id );
	return uint32_t( world->bodies.size() - 1 );
}

bool AddJoint( bench_world* handle, const bench_joint* joint )
{
	JoltWorld* world = reinterpret_cast<JoltWorld*>( handle );
	if ( joint->body_a >= world->bodies.size() || joint->body_b >= world->bodies.size() )
	{
		return false;
	}
	JPH::RMat44 frameA = world->FrameInWorld( joint->body_a, joint->frame_a );
	JPH::RMat44 frameB = world->FrameInWorld( joint->body_b, joint->frame_b );
	JPH::BodyInterface& bodies = world->Bodies();
	JPH::BodyID a = world->bodies[joint->body_a];
	JPH::BodyID b = world->bodies[joint->body_b];
	JPH::TwoBodyConstraint* constraint = nullptr;
	switch ( joint->kind )
	{
		case BENCH_SPHERICAL:
		{
			JPH::PointConstraintSettings settings;
			settings.mSpace = JPH::EConstraintSpace::WorldSpace;
			settings.mPoint1 = frameA.GetTranslation();
			settings.mPoint2 = frameB.GetTranslation();
			constraint = bodies.CreateConstraint( &settings, a, b );
			break;
		}
		case BENCH_REVOLUTE:
		{
			JPH::HingeConstraintSettings settings;
			settings.mSpace = JPH::EConstraintSpace::WorldSpace;
			settings.mPoint1 = frameA.GetTranslation();
			settings.mPoint2 = frameB.GetTranslation();
			settings.mHingeAxis1 = frameA.GetAxisZ();
			settings.mHingeAxis2 = frameB.GetAxisZ();
			settings.mNormalAxis1 = frameA.GetAxisX();
			settings.mNormalAxis2 = frameB.GetAxisX();
			settings.mMotorSettings.SetTorqueLimit( joint->max_motor_torque );
			JPH::HingeConstraint* hinge = static_cast<JPH::HingeConstraint*>( bodies.CreateConstraint( &settings, a, b ) );
			if ( hinge != nullptr && joint->motor )
			{
				hinge->SetMotorState( JPH::EMotorState::Velocity );
				hinge->SetTargetAngularVelocity( joint->motor_speed );
			}
			constraint = hinge;
			break;
		}
		default:
			return false;
	}
	if ( constraint == nullptr )
	{
		return false;
	}
	world->system.AddConstraint( constraint );
	return true;
}

void Prepare( bench_world* handle )
{
	JoltWorld* world = reinterpret_cast<JoltWorld*>( handle );
	if ( world->prepared )
	{
		return;
	}
	world->prepared = true;
	if ( world->pending.empty() )
	{
		return;
	}
	JPH::BodyInterface& bodies = world->Bodies();
	JPH::BodyInterface::AddState state = bodies.AddBodiesPrepare( world->pending.data(), int( world->pending.size() ) );
	bodies.AddBodiesFinalize( world->pending.data(), int( world->pending.size() ), state, JPH::EActivation::Activate );
	world->pending.clear();
	world->system.OptimizeBroadPhase();
}

void MoveKinematic( bench_world* handle, uint32_t body, bench_pose target, float time_step )
{
	JoltWorld* world = reinterpret_cast<JoltWorld*>( handle );
	world->Bodies().MoveKinematic( world->bodies[body], JPH::RVec3( ToVec( target.p ) ), ToQuat( target.q ), time_step );
}

void Step( bench_world* handle, float time_step )
{
	JoltWorld* world = reinterpret_cast<JoltWorld*>( handle );
	if ( world->system.Update( time_step, 1, &world->temp, &world->jobs ) != JPH::EPhysicsUpdateError::None )
	{
		std::fputs( "Jolt dropped contacts or pairs; raise the world capacity\n", stderr );
		std::abort();
	}
}

bench_pose Pose( const bench_world* handle, uint32_t body )
{
	const JoltWorld* world = reinterpret_cast<const JoltWorld*>( handle );
	const JPH::BodyInterface& bodies = world->Bodies();
	JPH::Vec3 p = JPH::Vec3( bodies.GetPosition( world->bodies[body] ) );
	JPH::Quat q = bodies.GetRotation( world->bodies[body] );
	return bench_pose{ FromVec( p ), bench_quat{ q.GetX(), q.GetY(), q.GetZ(), q.GetW() } };
}

uint32_t AwakeBodies( const bench_world* handle )
{
	const JoltWorld* world = reinterpret_cast<const JoltWorld*>( handle );
	return world->system.GetNumActiveBodies( JPH::EBodyType::RigidBody );
}

const bench_engine kEngine = {
	Create, Destroy, AddBody, AddJoint, Prepare, MoveKinematic, Step, Pose, AwakeBodies,
};

}

extern "C" const bench_engine* jolt_bench_engine( void )
{
	return &kEngine;
}
