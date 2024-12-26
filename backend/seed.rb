require_relative './database'

module Seed
  module_function

  def execute
    db = Database.connect

    db.exec "INSERT INTO users (name, guest) VALUES ('Alice', false)"
    db.exec 'INSERT INTO games (state) VALUES ($1)', ['{"map":"X---\n----\n----\n---X"}']
    db.exec 'INSERT INTO games (state) VALUES ($1)', ['{"map":"X---\n----\n----\n---XX---\n----\n----\n---X"}']

    db.close
  end
end
