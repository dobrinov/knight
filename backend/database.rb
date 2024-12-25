require 'pg'
require 'connection_pool'

module Database
  module_function

  NAME = 'knight'.freeze

  def connection_pool
    ConnectionPool.new(size: 5, timeout: 5) { connect }
  end

  def connect
    PG::Connection.new dbname: NAME, user: 'root'
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
end
