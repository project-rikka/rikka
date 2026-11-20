#include "engine.h"

#include "BulletCollision/CollisionDispatch/btManifoldResult.h"
#include "LinearMath/btTransformUtil.h"
#include "btBulletDynamicsCommon.h"

#include <algorithm>
#include <map>
#include <memory>
#include <mutex>
#include <new>
#include <utility>
#include <vector>

namespace
{

constexpr btScalar kTimeToSleep = 0.5f;

std::once_flag gConfigured;

btScalar GeometricFriction( const btCollisionObject* a, const btCollisionObject* b )
{
	return btSqrt( a->getFriction() * b->getFriction() );
}

btScalar MaxRestitution( const btCollisionObject* a, const btCollisionObject* b )
{
	return std::max( a->getRestitution(), b->getRestitution() );
}

void Configure()
{
	std::call_once( gConfigured, [] {
		gCalculateCombinedFrictionCallback = GeometricFriction;
		gCalculateCombinedRestitutionCallback = MaxRestitution;
		gDeactivationTime = kTimeToSleep;
	} );
}

btVector3 ToVec( bench_vec3 v )
{
	return btVector3( v.x, v.y, v.z );
}

btQuaternion ToQuat( bench_quat q )
{
	return btQuaternion( q.x, q.y, q.z, q.w ).normalized();
}

btTransform ToTransform( bench_pose pose )
{
	return btTransform( ToQuat( pose.q ), ToVec( pose.p ) );
}

bench_vec3 FromVec( const btVector3& v )
{
	return bench_vec3{ float( v.x() ), float( v.y() ), float( v.z() ) };
}

struct Mesh
{
	std::vector<btVector3> vertices;
	std::vector<int> indices;
	std::unique_ptr<btTriangleIndexVertexArray> array;
};

struct Child
{
	btCollisionShape* shape;
	btTransform local;
	float mass;
};

struct Body
{
	btRigidBody* rigid;
	btTransform originFromCenter;
	btVector3 linearVelocity;
	btVector3 angularVelocity;
	bool kinematic;
	bool steered;
};

struct BulletWorld
{
	explicit BulletWorld( const bench_world_def& def )
		: dispatcher( &configuration )
		, world( &dispatcher, &broadphase, &solver, &configuration )
		, sleeping( def.sleeping )
		, sleepThreshold( def.sleep_threshold )
	{
		world.setGravity( ToVec( def.gravity ) );
		bodies.reserve( def.max_bodies );
	}

	~BulletWorld()
	{
		for ( int i = world.getNumConstraints() - 1; i >= 0; --i )
		{
			world.removeConstraint( world.getConstraint( i ) );
		}
		for ( Body& body : bodies )
		{
			world.removeRigidBody( body.rigid );
			delete body.rigid->getMotionState();
			delete body.rigid;
		}
	}

	btDefaultCollisionConfiguration configuration;
	btCollisionDispatcher dispatcher;
	btDbvtBroadphase broadphase;
	btSequentialImpulseConstraintSolver solver;
	btDiscreteDynamicsWorld world;
	bool sleeping;
	btScalar sleepThreshold;
	std::vector<Body> bodies;
	std::vector<std::unique_ptr<btCollisionShape>> owned;
	std::vector<std::unique_ptr<Mesh>> meshes;
	std::vector<std::unique_ptr<btTypedConstraint>> joints;
	std::vector<uint32_t> kinematics;
	std::vector<std::pair<btHingeConstraint*, btScalar>> motors;
	std::map<uint64_t, std::pair<btCollisionShape*, btVector3>> cache;

	btCollisionShape* Keep( btCollisionShape* shape )
	{
		owned.emplace_back( shape );
		return shape;
	}

	std::pair<btCollisionShape*, btVector3> Geometry( const bench_shape& shape )
	{
		auto found = cache.find( shape.key );
		if ( found != cache.end() )
		{
			return found->second;
		}
		std::pair<btCollisionShape*, btVector3> made = MakeGeometry( shape );
		if ( made.first != nullptr )
		{
			cache.emplace( shape.key, made );
		}
		return made;
	}

	std::pair<btCollisionShape*, btVector3> MakeGeometry( const bench_shape& shape )
	{
		btVector3 zero( 0.0f, 0.0f, 0.0f );
		switch ( shape.geometry )
		{
			case BENCH_CUBOID:
				return { Keep( new btBoxShape( ToVec( shape.half_extents ) ) ), zero };
			case BENCH_SPHERE:
				return { Keep( new btSphereShape( shape.radius ) ), zero };
			case BENCH_HULL:
			{
				btConvexHullShape* hull = new btConvexHullShape();
				for ( uint32_t i = 0; i < shape.point_count; ++i )
				{
					hull->addPoint( ToVec( shape.points[i] ) - ToVec( shape.centroid ), false );
				}
				hull->recalcLocalAabb();
				hull->optimizeConvexHull();
				return { Keep( hull ), ToVec( shape.centroid ) };
			}
			case BENCH_MESH:
			{
				std::unique_ptr<Mesh> mesh = std::make_unique<Mesh>();
				mesh->vertices.reserve( shape.point_count );
				for ( uint32_t i = 0; i < shape.point_count; ++i )
				{
					mesh->vertices.push_back( ToVec( shape.points[i] ) );
				}
				mesh->indices.assign( shape.indices, shape.indices + shape.index_count );
				mesh->array = std::make_unique<btTriangleIndexVertexArray>(
					int( shape.index_count / 3 ), mesh->indices.data(), int( 3 * sizeof( int ) ), int( shape.point_count ),
					reinterpret_cast<btScalar*>( mesh->vertices.data() ), int( sizeof( btVector3 ) ) );
				btBvhTriangleMeshShape* triangles = new btBvhTriangleMeshShape( mesh->array.get(), true );
				meshes.push_back( std::move( mesh ) );
				return { Keep( triangles ), zero };
			}
			default:
				return { nullptr, zero };
		}
	}

	bool Children( const bench_body& body, std::vector<Child>& children )
	{
		for ( uint32_t i = 0; i < body.shape_count; ++i )
		{
			const bench_shape& shape = body.shapes[i];
			std::pair<btCollisionShape*, btVector3> geometry = Geometry( shape );
			if ( geometry.first == nullptr )
			{
				return false;
			}
			btTransform local = ToTransform( shape.pose );
			local.setOrigin( local.getOrigin() + local.getBasis() * geometry.second );
			children.push_back( Child{ geometry.first, local, shape.density * shape.volume } );
		}
		return true;
	}
};

bool IsIdentity( const btTransform& transform )
{
	return transform.getOrigin().fuzzyZero() && transform.getRotation() == btQuaternion::getIdentity();
}

bench_world* Create( const bench_world_def* def )
{
	Configure();
	return reinterpret_cast<bench_world*>( new ( std::nothrow ) BulletWorld( *def ) );
}

void Destroy( bench_world* handle )
{
	BulletWorld* world = reinterpret_cast<BulletWorld*>( handle );
	delete world;
}

uint32_t AddBody( bench_world* handle, const bench_body* body )
{
	BulletWorld* world = reinterpret_cast<BulletWorld*>( handle );
	std::vector<Child> children;
	if ( body->shape_count == 0 || !world->Children( *body, children ) )
	{
		return BENCH_INVALID_BODY;
	}
	bool dynamic = body->motion == BENCH_DYNAMIC;
	btCollisionShape* shape = nullptr;
	btTransform centerFromOrigin = btTransform::getIdentity();
	btScalar mass = 0.0f;
	btVector3 inertia( 0.0f, 0.0f, 0.0f );
	if ( children.size() == 1 && IsIdentity( children[0].local ) )
	{
		shape = children[0].shape;
		if ( dynamic )
		{
			mass = children[0].mass;
			shape->calculateLocalInertia( mass, inertia );
		}
	}
	else
	{
		btCompoundShape* placed = new btCompoundShape( true, int( children.size() ) );
		for ( const Child& child : children )
		{
			placed->addChildShape( child.local, child.shape );
		}
		if ( dynamic )
		{
			std::vector<btScalar> masses;
			for ( const Child& child : children )
			{
				masses.push_back( child.mass );
				mass += child.mass;
			}
			btTransform principal;
			placed->calculatePrincipalAxisTransform( masses.data(), principal, inertia );
			btCompoundShape* shifted = new btCompoundShape( true, int( children.size() ) );
			for ( const Child& child : children )
			{
				shifted->addChildShape( principal.inverse() * child.local, child.shape );
			}
			delete placed;
			placed = shifted;
			centerFromOrigin = principal;
		}
		shape = world->Keep( placed );
	}
	btTransform origin = ToTransform( body->pose );
	btDefaultMotionState* motion = new btDefaultMotionState( origin * centerFromOrigin );
	btRigidBody::btRigidBodyConstructionInfo info( mass, motion, shape, inertia );
	info.m_friction = body->shapes[0].friction;
	info.m_restitution = body->shapes[0].restitution;
	info.m_rollingFriction = body->shapes[0].rolling_resistance;
	btRigidBody* rigid = new btRigidBody( info );
	bool kinematic = body->motion == BENCH_KINEMATIC;
	if ( kinematic )
	{
		rigid->setCollisionFlags( rigid->getCollisionFlags() | btCollisionObject::CF_KINEMATIC_OBJECT );
		rigid->setActivationState( DISABLE_DEACTIVATION );
	}
	else if ( dynamic )
	{
		rigid->setLinearVelocity( ToVec( body->linear_velocity ) );
		rigid->setAngularVelocity( ToVec( body->angular_velocity ) );
		if ( !world->sleeping || !body->sleep )
		{
			rigid->setActivationState( DISABLE_DEACTIVATION );
		}
		else
		{
			btScalar threshold = world->sleepThreshold;
			btVector3 center;
			btScalar extent;
			shape->getBoundingSphere( center, extent );
			rigid->setSleepingThresholds( threshold, threshold / std::max( extent, btScalar( 0.01f ) ) );
		}
	}
	world->world.addRigidBody( rigid, int( body->shapes[0].category ), int( body->shapes[0].mask ) );
	world->bodies.push_back( Body{ rigid, centerFromOrigin.inverse(), ToVec( body->linear_velocity ),
								   ToVec( body->angular_velocity ), kinematic, false } );
	uint32_t index = uint32_t( world->bodies.size() - 1 );
	if ( kinematic )
	{
		world->kinematics.push_back( index );
	}
	return index;
}

bool AddJoint( bench_world* handle, const bench_joint* joint )
{
	BulletWorld* world = reinterpret_cast<BulletWorld*>( handle );
	if ( joint->body_a >= world->bodies.size() || joint->body_b >= world->bodies.size() )
	{
		return false;
	}
	const Body& a = world->bodies[joint->body_a];
	const Body& b = world->bodies[joint->body_b];
	btTransform frameA = a.originFromCenter.inverse() * ToTransform( joint->frame_a );
	btTransform frameB = b.originFromCenter.inverse() * ToTransform( joint->frame_b );
	btTypedConstraint* constraint = nullptr;
	switch ( joint->kind )
	{
		case BENCH_SPHERICAL:
			constraint = new btPoint2PointConstraint( *a.rigid, *b.rigid, frameA.getOrigin(), frameB.getOrigin() );
			break;
		case BENCH_REVOLUTE:
		{
			btHingeConstraint* hinge = new btHingeConstraint( *a.rigid, *b.rigid, frameA, frameB );
			if ( joint->motor )
			{
				hinge->enableAngularMotor( true, joint->motor_speed, 0.0f );
				world->motors.emplace_back( hinge, joint->max_motor_torque );
			}
			constraint = hinge;
			break;
		}
		default:
			return false;
	}
	world->joints.emplace_back( constraint );
	world->world.addConstraint( constraint, true );
	return true;
}

void Prepare( bench_world* handle )
{
	BulletWorld* world = reinterpret_cast<BulletWorld*>( handle );
	(void)world;
}

void MoveKinematic( bench_world* handle, uint32_t body, bench_pose target, float time_step )
{
	BulletWorld* world = reinterpret_cast<BulletWorld*>( handle );
	(void)time_step;
	Body& entry = world->bodies[body];
	entry.steered = true;
	entry.rigid->getMotionState()->setWorldTransform( ToTransform( target ) * entry.originFromCenter.inverse() );
}

void Step( bench_world* handle, float time_step )
{
	BulletWorld* world = reinterpret_cast<BulletWorld*>( handle );
	for ( uint32_t index : world->kinematics )
	{
		Body& body = world->bodies[index];
		if ( body.steered )
		{
			continue;
		}
		btTransform current;
		body.rigid->getMotionState()->getWorldTransform( current );
		btTransform next;
		btTransformUtil::integrateTransform( current, body.linearVelocity, body.angularVelocity, time_step, next );
		body.rigid->getMotionState()->setWorldTransform( next );
	}
	for ( const std::pair<btHingeConstraint*, btScalar>& motor : world->motors )
	{
		motor.first->setMaxMotorImpulse( motor.second * time_step );
	}
	world->world.stepSimulation( time_step, 1, time_step );
}

bench_pose Pose( const bench_world* handle, uint32_t body )
{
	const BulletWorld* world = reinterpret_cast<const BulletWorld*>( handle );
	const Body& entry = world->bodies[body];
	btTransform origin = entry.rigid->getWorldTransform() * entry.originFromCenter;
	btQuaternion q = origin.getRotation();
	return bench_pose{ FromVec( origin.getOrigin() ),
					   bench_quat{ float( q.x() ), float( q.y() ), float( q.z() ), float( q.w() ) } };
}

uint32_t AwakeBodies( const bench_world* handle )
{
	const BulletWorld* world = reinterpret_cast<const BulletWorld*>( handle );
	uint32_t awake = 0;
	for ( const Body& body : world->bodies )
	{
		if ( !body.rigid->isStaticOrKinematicObject() && body.rigid->isActive() )
		{
			++awake;
		}
	}
	return awake;
}

const bench_engine kEngine = {
	Create, Destroy, AddBody, AddJoint, Prepare, MoveKinematic, Step, Pose, AwakeBodies,
};

}

extern "C" const bench_engine* bullet_bench_engine( void )
{
	return &kEngine;
}
