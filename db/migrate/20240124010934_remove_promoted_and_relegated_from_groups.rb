class RemovePromotedAndRelegatedFromGroups < ActiveRecord::Migration[6.1]
  def change
    remove_column :groups, :promoted, :int
    remove_column :groups, :relegated, :int
  end
end
