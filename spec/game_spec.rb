require 'spec_helper'
require_relative '../lib/main'

describe 'Game' do
  it 'plays' do
    map =
      <<~MAP
        X---
        ----
        ----
        ---X
      MAP

    game = Game.new map:, min_players: 2, max_players: 2

    player_one = game.add_player 'Victor'
    player_two = game.add_player 'Alexander'

    game.start

    game.move_piece from: [0, 0], to: [1, 1]
    game.move_piece from: [1, 1], to: [2, 2]
    game.move_piece from: [2, 2], to: [3, 3]

    p game
  end

  describe 'ending turns' do
    it 'switches between players' do
      map =
        <<~MAP
          X-
          -X
        MAP

      game = Game.new map:, min_players: 2, max_players: 2

      victor = game.add_player 'Victor'
      alexander = game.add_player 'Alexander'

      game.start
      game.current_player.should eq victor
      game.end_turn
      game.current_player.should eq alexander
      game.end_turn
      game.current_player.should eq victor
    end

    it 'distributes resources from occupied blocks'
    end
  end
end
