require 'spec_helper'
require_relative '../lib/main'

describe Map do
  it 'parses a map' do
    map =
      Map.parse <<~MAP
        X-WS
        IG-X
      MAP

    map.width.should eq 4
    map.height.should eq 2

    board = map.to_board(2)

    block = board[0][0]
    block.player_id.should eq 0
    block.piece.should be_a King
    block.resource.should be_nil

    block = board[0][1]
    block.player_id.should be_nil
    block.piece.should be_nil
    block.resource.should be_nil

    block = board[0][2]
    block.player_id.should be_nil
    block.piece.should be_nil
    block.resource.should eq Resource::WOOD

    block = board[0][3]
    block.player_id.should be_nil
    block.piece.should be_nil
    block.resource.should eq Resource::STONE

    block = board[1][0]
    block.player_id.should be_nil
    block.piece.should be_nil
    block.resource.should eq Resource::IRON

    block = board[1][1]
    block.player_id.should be_nil
    block.piece.should be_nil
    block.resource.should eq Resource::GOLD

    block = board[1][2]
    block.player_id.should be_nil
    block.piece.should be_nil
    block.resource.should be_nil

    block = board[1][3]
    block.player_id.should eq 1
    block.piece.should be_a King
    block.resource.should be_nil
  end
end
