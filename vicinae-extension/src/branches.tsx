import BranchList from './BranchList';

export default function Branches() {
  return (
    <BranchList
      navigationTitle="Branches"
      emptyTitle="No Branches"
      emptyDescription="No branches found in stored repositories."
    />
  );
}
