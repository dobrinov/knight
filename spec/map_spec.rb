require 'spec_helper'
require_relative '../lib/main'

describe Map do
  it 'parses a map' do
    map =
      Map.parse <<~MAP
        SGG
        GGG
        GGS
      MAP
  end
end
