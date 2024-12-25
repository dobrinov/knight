require 'sinatra'
require_relative './database'
require_relative './lib/main'

DB = Database.connection_pool

before do
  content_type :json
  headers 'Access-Control-Allow-Origin' => '*', 'Access-Control-Allow-Methods' => %w[OPTIONS GET POST]
end

post '/api/users' do
  # Create a user
end

post '/api/guests' do
  request.body.rewind
  body = JSON.parse request.body.read

  DB.with do |psql|
    psql.exec_params('INSERT INTO users (name, guest) VALUES ($1, $2) RETURNING *', [body['name'], true])
  end

  # Create guest user and return JWT token
  'thisshouldbeaJWToken'
end

get '/api/lobby' do
  result =
    DB.with do |psql|
      psql.exec('SELECT * FROM games WHERE started_at IS NULL')
    end

  result.values.to_json
end

post '/api/games' do
  map =
    <<~MAP
      X---
      ----
      ----
      ---X
    MAP

  lobby = Lobby.new map: Map.parse(map)

  result =
    DB.with do |psql|
      psql.exec_params('INSERT INTO games (state) VALUES ($1) RETURNING *', [lobby.to_json])
    end

  result.values.to_s
end

get '/api/games/:id' do
  result =
    DB.with do |psql|
      psql.exec_params('SELECT * FROM games WHERE id = $1', [params[:id]])
    end

  state = JSON.parse result[0]['state']
  map = state['map']
  players = state['players']

  result[0].to_json
end

post '/api/games/:id/join' do
  # Get user from JWT token

  result =
    DB.with do |psql|
      psql.exec_params('SELECT * FROM games WHERE id = $1', [params[:id]])
    end
end
