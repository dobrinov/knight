require 'pg'
require 'connection_pool'

module Database
  NAME = 'knight'.freeze

  module_function

  def exec(query)
    connection_pool.with do |psql|
      psql.type_map_for_results = PG::BasicTypeMapForResults.new psql
      psql.exec(query).entries
    end
  end

  def exec_params(query, params)
    connection_pool.with do |psql|
      psql.type_map_for_results = PG::BasicTypeMapForResults.new psql
      psql.exec_params(query, params).entries
    end
  end

  def create
    db = PG::Connection.new
    db.exec "CREATE DATABASE #{NAME}"
    db.close
  end

  def drop
    db = PG::Connection.new
    db.exec "DROP DATABASE #{NAME}"
    db.close
  end

  def migrate
    db = connect

    db.exec <<~SQL
      CREATE TABLE users (
        id SERIAL PRIMARY KEY,
        name VARCHAR(255) NOT NULL,
        guest BOOLEAN NOT NULL DEFAULT FALSE,
        created_at TIMESTAMP NOT NULL DEFAULT NOW(),
        updated_at TIMESTAMP NOT NULL DEFAULT NOW()
      );

      CREATE TABLE games (
        id SERIAL PRIMARY KEY,
        state JSONB NOT NULL,
        started_at TIMESTAMP,
        completed_at TIMESTAMP,
        created_at TIMESTAMP NOT NULL DEFAULT NOW(),
        updated_at TIMESTAMP NOT NULL DEFAULT NOW()
      );

      CREATE TABLE game_participations (
        id SERIAL PRIMARY KEY,
        game_id INTEGER NOT NULL REFERENCES games(id),
        user_id INTEGER NOT NULL REFERENCES users(id),
        joined_at TIMESTAMP NOT NULL DEFAULT NOW()
      );
    SQL

    db.close
  end

  def connection_pool
    @connection_pool ||= ConnectionPool.new(size: 5, timeout: 5) { connect }
  end

  def connect
    PG::Connection.new dbname: NAME, user: 'root'
  end
end
