require 'spec_helper'
require_relative '../lib/main'

describe 'Game' do
  it 'plays' do
    game = Game.new min_players: 2, max_players: 2
  
    player_one = game.add_player 'Victor'
    player_two = game.add_player 'Alexander'
  
    game.start
  
    game.move_piece from: [0, 0], to: [1, 1]
    game.move_piece from: [1, 1], to: [2, 2]
    game.move_piece from: [2, 2], to: [3, 3]
    game.move_piece from: [2, 2], to: [3, 3]
  
    p game
  end
end
