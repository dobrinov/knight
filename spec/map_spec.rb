require 'spec_helper'
require_relative '../lib/main'

describe Map do
  it 'foo' do
    map =
      Map.parse <<~MAP
        SGG
        GGG
        GGS
      MAP

    puts map.to_board(2).to_s
  end
end
